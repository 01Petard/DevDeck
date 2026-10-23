use crate::contracts::{CommandError, CommandErrorCode, PortListener};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv6Addr};
use std::process::{Output, Stdio};
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

const MAX_STDOUT: usize = 8 * 1024 * 1024;
const MAX_STDERR: usize = 8 * 1024;
static SCAN_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn scan_listeners() -> Result<Vec<PortListener>, CommandError> {
    let _scan = SCAN_LOCK.try_lock().map_err(|_| CommandError {
        code: CommandErrorCode::ScanFailed,
        message: "正在扫描端口，请稍后刷新。".into(),
    })?;
    let child = Command::new("/usr/sbin/lsof")
        .args(["-nP", "+c", "0", "-iTCP", "-sTCP:LISTEN", "-F0pcftn"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| CommandError {
            code: CommandErrorCode::ScanFailed,
            message: "无法执行系统端口扫描工具；结果未更新。".into(),
        })?;
    let output = read_process_output(child, Duration::from_secs(5)).await?;
    let listeners = listeners_from_output(output)?;
    if listeners.is_empty() {
        return Ok(listeners);
    }
    tokio::task::spawn_blocking(move || enrich_processes(listeners))
        .await
        .map_err(|_| CommandError {
            code: CommandErrorCode::InternalError,
            message: "无法读取进程信息；结果未更新。".into(),
        })
}

async fn read_limited(reader: impl AsyncRead + Unpin, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "scan output limit exceeded",
        ));
    }
    Ok(bytes)
}

async fn read_process_output(mut child: Child, deadline: Duration) -> Result<Output, CommandError> {
    let stdout = child.stdout.take().ok_or_else(|| CommandError {
        code: CommandErrorCode::InternalError,
        message: "无法读取扫描输出。".into(),
    })?;
    let stderr = child.stderr.take().ok_or_else(|| CommandError {
        code: CommandErrorCode::InternalError,
        message: "无法读取扫描状态。".into(),
    })?;
    let result = tokio::time::timeout(deadline, async {
        let (stdout, stderr, status) = tokio::try_join!(
            read_limited(stdout, MAX_STDOUT),
            read_limited(stderr, MAX_STDERR),
            child.wait(),
        )?;
        Ok::<_, std::io::Error>(Output {
            status,
            stdout,
            stderr,
        })
    })
    .await;
    match result {
        Ok(Ok(output)) => Ok(output),
        failed => {
            // 这里只清理本次启动的 lsof 子进程，不向任何被扫描进程发送信号。
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_millis(250), child.wait()).await;
            if failed.is_err() {
                Err(CommandError {
                    code: CommandErrorCode::ScanTimeout,
                    message: "端口扫描超时；结果未更新。".into(),
                })
            } else {
                Err(CommandError {
                    code: CommandErrorCode::ScanFailed,
                    message: "扫描输出超过限制或无法读取；结果未更新。".into(),
                })
            }
        }
    }
}

fn enrich_processes(mut listeners: Vec<PortListener>) -> Vec<PortListener> {
    let pids: Vec<_> = listeners
        .iter()
        .map(|listener| Pid::from_u32(listener.pid))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .without_tasks(),
    );
    for listener in &mut listeners {
        if let Some(process) = system.process(Pid::from_u32(listener.pid)) {
            if !process.name().is_empty() {
                listener.process_name = process.name().to_string_lossy().into_owned();
            }
            listener.process_start_time = Some(process.start_time()).filter(|time| *time > 0);
            listener.executable_path = process
                .exe()
                .filter(|path| !path.as_os_str().is_empty())
                .map(|path| path.to_string_lossy().into_owned());
        }
    }
    listeners
}

#[derive(Default)]
struct ParsedListeners {
    listeners: Vec<PortListener>,
    skipped_records: usize,
    processes_seen: usize,
}

#[derive(Default)]
struct SocketFields {
    active: bool,
    ip_version: Option<u8>,
    name: Option<String>,
}

fn parse_lsof_output(output: &[u8]) -> ParsedListeners {
    let mut parsed = ParsedListeners::default();
    let mut pid = None;
    let mut process_name = String::new();
    let mut socket = SocketFields::default();

    for mut field in output.split(|byte| *byte == 0) {
        while field.first() == Some(&b'\n') {
            field = &field[1..];
        }
        let Some((&tag, value)) = field.split_first() else {
            continue;
        };
        match tag {
            b'p' => {
                append_socket(std::mem::take(&mut socket), pid, &process_name, &mut parsed);
                pid = std::str::from_utf8(value).ok().and_then(|text| {
                    text.bytes()
                        .all(|byte| byte.is_ascii_digit())
                        .then(|| text.parse::<u32>().ok())
                        .flatten()
                        .filter(|pid| *pid > 0)
                });
                process_name.clear();
                if pid.is_some() {
                    parsed.processes_seen += 1;
                }
            }
            b'c' => process_name = String::from_utf8_lossy(value).into_owned(),
            b'f' => {
                append_socket(std::mem::take(&mut socket), pid, &process_name, &mut parsed);
                socket.active = true;
            }
            b't' => {
                socket.ip_version = match value {
                    b"IPv4" => Some(4),
                    b"IPv6" => Some(6),
                    _ => None,
                }
            }
            b'n' => socket.name = Some(String::from_utf8_lossy(value).into_owned()),
            _ => {}
        }
    }
    append_socket(socket, pid, &process_name, &mut parsed);
    let mut seen = HashSet::new();
    parsed.listeners.retain(|listener| {
        seen.insert((
            listener.pid,
            listener.address.clone(),
            listener.port,
            listener.ip_version,
            listener.protocol,
        ))
    });
    parsed.listeners.sort_by(|a, b| {
        (a.port, a.pid, &a.address, a.ip_version).cmp(&(b.port, b.pid, &b.address, b.ip_version))
    });
    parsed
}

fn append_socket(socket: SocketFields, pid: Option<u32>, name: &str, parsed: &mut ParsedListeners) {
    if !socket.active {
        return;
    }
    let address_and_port = socket.name.as_deref().and_then(parse_socket_name);
    let (Some(pid), Some((address, port))) = (pid, address_and_port) else {
        parsed.skipped_records += 1;
        return;
    };
    parsed.listeners.push(PortListener {
        pid,
        process_name: name.to_owned(),
        process_start_time: None,
        executable_path: None,
        protocol: "TCP",
        address,
        port,
        ip_version: socket.ip_version,
    });
}

fn parse_socket_name(name: &str) -> Option<(String, u16)> {
    let (address, port) = name.rsplit_once(':')?;
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let port = port.parse::<u16>().ok().filter(|port| *port > 0)?;
    let address = if address.starts_with('[') {
        address.strip_prefix('[')?.strip_suffix(']')?
    } else {
        address
    };
    let valid_scope = address.rsplit_once('%').is_some_and(|(ip, scope)| {
        ip.parse::<Ipv6Addr>().is_ok()
            && !scope.is_empty()
            && scope
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    });
    if address != "*" && address.parse::<IpAddr>().is_err() && !valid_scope {
        return None;
    }
    Some((address.to_owned(), port))
}

fn listeners_from_output(output: Output) -> Result<Vec<PortListener>, CommandError> {
    if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
        return Ok(Vec::new());
    }
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(CommandError {
            code: CommandErrorCode::ScanFailed,
            message: "端口扫描失败，可能存在权限限制；结果未更新。".into(),
        });
    }
    let parsed = parse_lsof_output(&output.stdout);
    if parsed.skipped_records > 0 {
        eprintln!(
            "DevDock: skipped {} malformed lsof socket records",
            parsed.skipped_records
        );
    }
    if parsed.listeners.is_empty()
        && (parsed.skipped_records > 0
            || (parsed.processes_seen == 0
                && output.stdout.iter().any(|byte| !byte.is_ascii_whitespace())))
    {
        return Err(CommandError {
            code: CommandErrorCode::ScanFailed,
            message: "端口扫描结果无法解析；结果未更新。".into(),
        });
    }
    Ok(parsed.listeners)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::CommandErrorCode;
    use std::os::unix::process::ExitStatusExt;

    fn output(code: i32, stdout: &[u8], stderr: &[u8]) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    #[test]
    fn parses_macos_record_groups_and_preserves_distinct_sockets() {
        let parsed = parse_lsof_output(include_bytes!("../tests/fixtures/macos-listeners.lsof"));
        let sockets: Vec<_> = parsed
            .listeners
            .iter()
            .map(|l| {
                (
                    l.port,
                    l.pid,
                    l.address.as_str(),
                    l.ip_version,
                    l.process_name.as_str(),
                )
            })
            .collect();
        assert_eq!(
            sockets,
            vec![
                (5173, 202, "::1", Some(6), "node"),
                (8080, 101, "*", Some(4), "java"),
                (8080, 101, "*", Some(6), "java"),
                (8080, 101, "127.0.0.1", Some(4), "java"),
                (8080, 303, "127.0.0.1", Some(4), "other"),
            ]
        );
        assert_eq!(parsed.skipped_records, 0);
    }

    #[test]
    fn skips_bad_sockets_without_losing_later_processes() {
        let parsed =
            parse_lsof_output(include_bytes!("../tests/fixtures/malformed-listeners.lsof"));
        assert_eq!(parsed.listeners.len(), 2);
        assert_eq!(parsed.listeners[0].port, 4000);
        assert_eq!(parsed.listeners[0].ip_version, None);
        assert_eq!(parsed.listeners[1].address, "fe80::1%lo0");
        assert_eq!(parsed.skipped_records, 6);
    }

    #[test]
    fn only_process_headers_and_empty_output_are_valid_empty_snapshots() {
        for bytes in [b"".as_slice(), b"p123\0cvanished\0\n"] {
            assert!(
                listeners_from_output(output(0, bytes, b""))
                    .unwrap()
                    .is_empty()
            );
        }
        assert!(
            listeners_from_output(output(1, b"", b""))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn execution_and_permission_failures_never_become_empty_success() {
        for failed in [
            output(1, b"", b"lsof: permission denied /sensitive/path"),
            output(2, b"", b""),
            output(0, b"", b"lsof: cannot inspect process"),
            output(1, b"p123\0cx\0\nf1\0tIPv4\0n*:8080\0\n", b""),
            output(0, b"not lsof output", b""),
            output(0, b"p123\0cx\0\nf1\0tIPv4\0ninvalid\0\n", b""),
        ] {
            let error = listeners_from_output(failed).unwrap_err();
            assert_eq!(error.code, CommandErrorCode::ScanFailed);
            assert!(!error.message.contains("sensitive"));
            assert_eq!(serde_json::to_value(error).unwrap()["code"], "SCAN_FAILED");
        }
    }

    #[test]
    fn listener_json_matches_camel_case_contract_and_null_metadata() {
        let parsed = parse_lsof_output(b"p123\0cnode\0\nf7\0tIPv6\0n[::1]:5173\0\n");
        assert_eq!(
            serde_json::to_value(&parsed.listeners).unwrap(),
            serde_json::json!([{
                "pid": 123, "processName": "node", "processStartTime": null,
                "executablePath": null, "protocol": "TCP", "address": "::1",
                "port": 5173, "ipVersion": 6
            }])
        );
    }

    #[test]
    fn socket_names_use_rightmost_port_and_reject_invalid_addresses() {
        for (raw, address, port) in [
            ("*:1", "*", 1),
            ("::1:65535", "::1", 65535),
            ("[::]:8080", "::", 8080),
            ("192.168.1.2:80", "192.168.1.2", 80),
        ] {
            assert_eq!(parse_socket_name(raw), Some((address.into(), port)));
        }
        for raw in [
            "*:0",
            "*:65536",
            "*:80x",
            "*: +80",
            "*:+80",
            ":80",
            "[::1:80",
            "host:80",
            "127.0.0.1:80->127.0.0.1:1234",
        ] {
            assert_eq!(parse_socket_name(raw), None, "{raw}");
        }
    }

    #[tokio::test]
    async fn bounded_read_accepts_exact_limit_and_rejects_oversized_output() {
        for limit in [MAX_STDOUT, MAX_STDERR] {
            let data = vec![b'x'; limit + 1];
            assert_eq!(
                read_limited(&data[..limit], limit).await.unwrap().len(),
                limit
            );
            assert!(read_limited(data.as_slice(), limit).await.is_err());
        }
    }

    #[tokio::test]
    async fn timeout_reaps_only_the_owned_scan_child() {
        let mut command = tokio::process::Command::new("/bin/sleep");
        command
            .arg("1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let child = command.spawn().unwrap();
        let pid = sysinfo::Pid::from_u32(child.id().unwrap());
        let started = std::time::Instant::now();
        let error = read_process_output(child, Duration::from_millis(20))
            .await
            .unwrap_err();
        assert_eq!(error.code, CommandErrorCode::ScanTimeout);
        assert_eq!(serde_json::to_value(error).unwrap()["code"], "SCAN_TIMEOUT");
        assert!(started.elapsed() < Duration::from_secs(2));
        let mut system = sysinfo::System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        assert!(system.process(pid).is_none());
    }

    #[tokio::test]
    async fn oversized_child_output_returns_scan_failure_without_hanging() {
        let child = tokio::process::Command::new("/usr/bin/head")
            .args(["-c", &(MAX_STDOUT + 1).to_string(), "/dev/zero"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let error = read_process_output(child, Duration::from_secs(2))
            .await
            .unwrap_err();
        assert_eq!(error.code, CommandErrorCode::ScanFailed);
    }

    #[tokio::test]
    async fn concurrent_scan_is_rejected_before_starting_another_child() {
        let _guard = SCAN_LOCK.lock().await;
        assert_eq!(
            scan_listeners().await.unwrap_err().code,
            CommandErrorCode::ScanFailed
        );
    }

    #[test]
    fn process_metadata_is_real_and_missing_process_keeps_fallback() {
        let mut listeners =
            parse_lsof_output(b"p123\0cfallback\0\nf1\0tIPv4\0n*:8080\0\n").listeners;
        listeners[0].pid = std::process::id();
        listeners.push(PortListener {
            pid: u32::MAX,
            ..listeners[0].clone()
        });
        let result = enrich_processes(listeners);
        assert!(result[0].process_start_time.is_some_and(|time| time > 0));
        assert!(
            result[0]
                .executable_path
                .as_deref()
                .is_some_and(|path| !path.is_empty())
        );
        assert_ne!(result[0].process_name, "fallback");
        assert_eq!(result[1].process_name, "fallback");
        assert_eq!(result[1].process_start_time, None);
        assert_eq!(result[1].executable_path, None);
    }

    struct TestService(std::process::Child);

    impl TestService {
        fn stop(&mut self) -> std::io::Result<()> {
            if self.0.try_wait()?.is_some() {
                return Ok(());
            }
            let status = std::process::Command::new("/bin/kill")
                .args(["-TERM", &self.0.id().to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            if !status.success() {
                return Err(std::io::Error::other("cannot stop owned test service"));
            }
            for _ in 0..200 {
                if self.0.try_wait()?.is_some() {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "owned test service did not exit",
            ))
        }
    }

    impl Drop for TestService {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }

    #[tokio::test]
    #[ignore = "macOS live acceptance: starts only its own Python service on 18765"]
    async fn live_scan_finds_owned_python_service_and_observes_its_removal() {
        assert_eq!(std::env::consts::OS, "macos");
        let reservation = std::net::TcpListener::bind("127.0.0.1:18765")
            .expect("18765 is occupied; do not touch the existing service");
        drop(reservation);
        let child = std::process::Command::new("python3")
            .args(["-m", "http.server", "18765", "--bind", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start owned Python test service");
        let mut service = TestService(child);
        let pid = service.0.id();
        let ready_until = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            assert!(
                service.0.try_wait().unwrap().is_none(),
                "owned service exited before listening"
            );
            if std::net::TcpStream::connect("127.0.0.1:18765").is_ok() {
                break;
            }
            assert!(
                std::time::Instant::now() < ready_until,
                "owned service did not start"
            );
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        let listeners = crate::commands::list_listeners().await.unwrap();
        let listener = listeners
            .iter()
            .find(|l| l.pid == pid && l.port == 18765)
            .expect("native handler must discover the owned process");
        assert_eq!(listener.address, "127.0.0.1");
        assert_eq!(listener.ip_version, Some(4));
        assert!(listener.process_start_time.is_some_and(|time| time > 0));
        assert!(listener.executable_path.is_some());
        let direct = std::process::Command::new("/usr/sbin/lsof")
            .args([
                "-nP",
                "-a",
                "-p",
                &pid.to_string(),
                "-iTCP:18765",
                "-sTCP:LISTEN",
                "-Fp",
            ])
            .output()
            .unwrap();
        assert!(direct.status.success());
        let direct_stdout = String::from_utf8(direct.stdout).unwrap();
        let direct_pids: Vec<_> = direct_stdout
            .lines()
            .filter(|line| line.starts_with('p'))
            .collect();
        assert_eq!(direct_pids, vec![format!("p{pid}")]);
        println!(
            "LIVE PASS: port=18765 address=127.0.0.1 pid={pid} IPv4; lsof PID matches; metadata available"
        );
        service.stop().unwrap();
        let after = crate::commands::list_listeners().await.unwrap();
        assert!(!after.iter().any(|l| l.pid == pid && l.port == 18765));
        println!(
            "LIVE PASS: owned service exited after SIGTERM cleanup; listener disappeared on rescan"
        );
    }
}

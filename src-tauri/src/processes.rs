use crate::contracts::{CommandError, CommandErrorCode, TerminateRequest, TerminateResult};
use std::time::Duration;
use sysinfo::{
    Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, Signal, System, UpdateKind,
};
use tokio::sync::Mutex;

static TERMINATION_LOCK: Mutex<()> = Mutex::const_new(());

fn validate_request(request: &TerminateRequest) -> Result<(), CommandError> {
    if request.pid > i32::MAX as u32 || request.process_start_time == 0 || request.port == 0 {
        return Err(CommandError {
            code: CommandErrorCode::InvalidArgument,
            message: "进程身份或端口参数无效，未发送信号。".into(),
        });
    }
    if request.pid <= 1 || request.pid == std::process::id() {
        return Err(CommandError {
            code: CommandErrorCode::ProcessProtected,
            message: "不能结束 DevDock 自身或系统特殊进程，未发送信号。".into(),
        });
    }
    Ok(())
}

fn validate_identity(
    expected: u64,
    observed: u64,
    same_owner: Option<bool>,
    zombie: bool,
) -> Result<(), CommandError> {
    let (code, message) = if zombie {
        (
            CommandErrorCode::ProcessNotFound,
            "原进程已退出，未发送信号。",
        )
    } else if observed == 0 || same_owner.is_none() {
        (
            CommandErrorCode::ProcessUnverifiable,
            "无法验证进程身份，未发送信号。",
        )
    } else if expected != observed {
        (
            CommandErrorCode::ProcessIdentityChanged,
            "进程身份已变化，未发送信号。请刷新并重新选择。",
        )
    } else if same_owner == Some(false) {
        (
            CommandErrorCode::PermissionDenied,
            "无法结束其他用户或权限不同的进程，未发送信号。",
        )
    } else {
        return Ok(());
    };
    Err(CommandError {
        code,
        message: message.into(),
    })
}

fn existence(pid: u32) -> Result<(), i32> {
    // Signal 0 only checks existence/permission. Callers have excluded non-positive/group PIDs.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0))
    }
}

fn verified_system(request: &TerminateRequest) -> Result<System, CommandError> {
    if let Err(code) = existence(request.pid) {
        return Err(signal_error(code));
    }
    let pid = Pid::from_u32(request.pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing()
            .with_user(UpdateKind::Always)
            .without_tasks(),
    );
    let process = system.process(pid).ok_or_else(|| {
        if existence(request.pid) == Err(libc::ESRCH) {
            signal_error(libc::ESRCH)
        } else {
            CommandError {
                code: CommandErrorCode::ProcessUnverifiable,
                message: "无法读取进程身份，未发送信号。".into(),
            }
        }
    })?;
    let own_uid = unsafe { libc::geteuid() };
    let same_owner = process
        .user_id()
        .zip(process.effective_user_id())
        .map(|(real, effective)| **real == own_uid && **effective == own_uid);
    validate_identity(
        request.process_start_time,
        process.start_time(),
        same_owner,
        process.status() == ProcessStatus::Zombie,
    )?;
    Ok(system)
}

fn signal_error(code: i32) -> CommandError {
    let (code, message) = match code {
        libc::ESRCH => (
            CommandErrorCode::ProcessNotFound,
            "进程已不存在，未发送信号。",
        ),
        libc::EPERM | libc::EACCES => (
            CommandErrorCode::PermissionDenied,
            "没有结束该进程的权限，未发送信号。",
        ),
        _ => (
            CommandErrorCode::SignalFailed,
            "未能发送 SIGTERM，请刷新确认当前状态。",
        ),
    };
    CommandError {
        code,
        message: message.into(),
    }
}

fn original_has_exited(
    expected: u64,
    presence: Result<(), i32>,
    observed: Option<(u64, bool)>,
) -> bool {
    if presence == Err(libc::ESRCH) {
        return true;
    }
    if presence.is_err() {
        return false;
    }
    observed.is_some_and(|(start, zombie)| start > 0 && (start != expected || zombie))
}

fn observe_exit(request: TerminateRequest) -> bool {
    let presence = existence(request.pid);
    if presence.is_err() {
        return original_has_exited(request.process_start_time, presence, None);
    }
    let pid = Pid::from_u32(request.pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    let observed = system.process(pid).map(|process| {
        (
            process.start_time(),
            process.status() == ProcessStatus::Zombie,
        )
    });
    original_has_exited(request.process_start_time, existence(request.pid), observed)
}

pub async fn terminate_listener(
    request: TerminateRequest,
) -> Result<TerminateResult, CommandError> {
    validate_request(&request)?;
    let _termination = TERMINATION_LOCK.try_lock().map_err(|_| CommandError {
        code: CommandErrorCode::InvalidArgument,
        message: "已有终止操作正在进行，请等待结果。".into(),
    })?;
    let unverifiable = || CommandError {
        code: CommandErrorCode::ProcessUnverifiable,
        message: "无法完成身份复核，未发送信号。请刷新后重试。".into(),
    };
    tokio::task::spawn_blocking(move || verified_system(&request).map(|_| ()))
        .await
        .map_err(|_| unverifiable())??;
    let listeners = crate::ports::scan_listeners()
        .await
        .map_err(|_| unverifiable())?;
    let matching: Vec<_> = listeners
        .iter()
        .filter(|row| row.pid == request.pid && row.port == request.port)
        .collect();
    if matching.is_empty() {
        return Err(CommandError {
            code: CommandErrorCode::ProcessIdentityChanged,
            message: "该进程的监听端口已变化，未发送信号。请刷新并重新选择。".into(),
        });
    }
    for listener in matching {
        validate_identity(
            request.process_start_time,
            listener.process_start_time.unwrap_or(0),
            Some(true),
            false,
        )?;
    }
    tokio::task::spawn_blocking(move || {
        let system = verified_system(&request)?;
        let process = system
            .process(Pid::from_u32(request.pid))
            .ok_or_else(unverifiable)?;
        match process.kill_with(Signal::Term) {
            Some(true) => Ok(()),
            Some(false) => Err(signal_error(
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
            )),
            None => Err(signal_error(0)),
        }
    })
    .await
    .map_err(|_| CommandError {
        code: CommandErrorCode::InternalError,
        message: "无法确认操作结果，请刷新检查进程状态。".into(),
    })??;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut process_exited = false;
    loop {
        let observation = tokio::time::timeout_at(
            deadline,
            tokio::task::spawn_blocking(move || observe_exit(request)),
        )
        .await;
        match observation {
            Ok(Ok(true)) => {
                process_exited = true;
                break;
            }
            Ok(Ok(false)) => {}
            _ => break,
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        tokio::time::sleep_until((now + Duration::from_millis(100)).min(deadline)).await;
    }
    Ok(TerminateResult {
        signal_sent: true,
        process_exited,
        message: if process_exited {
            "已发送 SIGTERM，并确认原进程已退出。"
        } else {
            "已发送 SIGTERM，2 秒内尚未确认原进程退出。"
        }
        .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_reject_special_self_and_group_pids_and_missing_start_time() {
        let request = TerminateRequest {
            pid: 12345,
            process_start_time: 100,
            port: 8080,
        };
        assert!(validate_request(&request).is_ok());
        for pid in [0, 1, std::process::id()] {
            assert_eq!(
                validate_request(&TerminateRequest { pid, ..request })
                    .unwrap_err()
                    .code,
                CommandErrorCode::ProcessProtected
            );
        }
        for invalid in [
            TerminateRequest {
                pid: i32::MAX as u32 + 1,
                ..request
            },
            TerminateRequest {
                process_start_time: 0,
                ..request
            },
            TerminateRequest { port: 0, ..request },
        ] {
            assert_eq!(
                validate_request(&invalid).unwrap_err().code,
                CommandErrorCode::InvalidArgument
            );
        }
        for raw in [
            r#"{"pid":2,"processStartTime":null,"port":8080}"#,
            r#"{"pid":2,"port":8080}"#,
            r#"{"pid":2,"processStartTime":"100","port":8080}"#,
            r#"{"pid":-1,"processStartTime":100,"port":8080}"#,
            r#"{"pid":2.5,"processStartTime":100,"port":8080}"#,
            r#"{"pid":2,"processStartTime":100,"port":65536}"#,
        ] {
            assert!(serde_json::from_str::<TerminateRequest>(raw).is_err());
        }
    }

    #[test]
    fn identity_requires_matching_known_start_time_and_current_user_ownership() {
        assert!(validate_identity(100, 100, Some(true), false).is_ok());
        for (observed, owner, zombie, code) in [
            (0, Some(true), false, CommandErrorCode::ProcessUnverifiable),
            (
                101,
                Some(true),
                false,
                CommandErrorCode::ProcessIdentityChanged,
            ),
            (100, None, false, CommandErrorCode::ProcessUnverifiable),
            (100, Some(false), false, CommandErrorCode::PermissionDenied),
            (100, Some(true), true, CommandErrorCode::ProcessNotFound),
        ] {
            assert_eq!(
                validate_identity(100, observed, owner, zombie)
                    .unwrap_err()
                    .code,
                code
            );
        }
    }

    #[test]
    fn exit_confirmation_distinguishes_reuse_and_zombies_from_unreadable_metadata() {
        assert!(original_has_exited(100, Err(libc::ESRCH), None));
        assert!(original_has_exited(100, Ok(()), Some((101, false))));
        assert!(original_has_exited(100, Ok(()), Some((100, true))));
        for (presence, observed) in [
            (Ok(()), Some((100, false))),
            (Ok(()), None),
            (Ok(()), Some((0, false))),
            (Err(libc::EPERM), None),
        ] {
            assert!(!original_has_exited(100, presence, observed));
        }
        assert_eq!(
            signal_error(libc::EPERM).code,
            CommandErrorCode::PermissionDenied
        );
        assert_eq!(
            signal_error(libc::ESRCH).code,
            CommandErrorCode::ProcessNotFound
        );
        assert_eq!(
            signal_error(libc::EINVAL).code,
            CommandErrorCode::SignalFailed
        );
        let result = serde_json::to_value(TerminateResult {
            signal_sent: true,
            process_exited: false,
            message: "sent".into(),
        })
        .unwrap();
        assert_eq!(
            result,
            serde_json::json!({"signalSent":true,"processExited":false,"message":"sent"})
        );
    }

    #[tokio::test]
    async fn concurrent_termination_is_rejected_before_process_io() {
        let _guard = TERMINATION_LOCK.lock().await;
        let result = terminate_listener(TerminateRequest {
            pid: std::process::id() + 1,
            process_start_time: 100,
            port: 8080,
        })
        .await;
        assert_eq!(result.unwrap_err().code, CommandErrorCode::InvalidArgument);
    }

    struct OwnedService {
        child: std::process::Child,
        port: u16,
    }
    impl OwnedService {
        fn start(delayed: bool) -> Self {
            use std::io::BufRead;
            let code = r#"
import socket, signal, threading, os, sys
listener = socket.socket()
listener.bind(('127.0.0.1', 0))
listener.listen()
def stop_later(*args):
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    threading.Timer(3.5, lambda: os._exit(0)).start()
if sys.argv[1] == 'delayed': signal.signal(signal.SIGTERM, stop_later)
print(listener.getsockname()[1], flush=True)
while True:
    connection, _ = listener.accept()
    connection.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n')
    connection.close()
"#;
            let mut child = std::process::Command::new("python3")
                .args(["-u", "-c", code, if delayed { "delayed" } else { "normal" }])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let mut port = String::new();
            std::io::BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut port)
                .unwrap();
            Self {
                child,
                port: port
                    .trim()
                    .parse()
                    .expect("owned Python service ready port"),
            }
        }
        async fn request(&self) -> TerminateRequest {
            let listeners = crate::commands::list_listeners().await.unwrap();
            let row = listeners
                .iter()
                .find(|row| row.pid == self.child.id() && row.port == self.port)
                .unwrap();
            TerminateRequest {
                pid: row.pid,
                process_start_time: row.process_start_time.unwrap(),
                port: row.port,
            }
        }
    }
    impl Drop for OwnedService {
        fn drop(&mut self) {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            let mut system = System::new();
            let pid = Pid::from_u32(self.child.id());
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().without_tasks(),
            );
            if let Some(process) = system.process(pid) {
                process.kill_with(Signal::Term);
            }
            let until = std::time::Instant::now() + Duration::from_secs(5);
            while std::time::Instant::now() < until {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            eprintln!("owned process cleanup did not complete within its deadline");
        }
    }

    #[tokio::test]
    #[ignore = "macOS live acceptance; sends SIGTERM only to its own temporary Python services"]
    async fn live_termination_rejects_changed_identity_and_port_and_observes_exit() {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(std::env::consts::OS, "macos");
        let mut service = OwnedService::start(false);
        let request = service.request().await;
        for invalid in [
            TerminateRequest {
                process_start_time: request.process_start_time + 1,
                ..request
            },
            TerminateRequest {
                port: if request.port == 65535 {
                    65534
                } else {
                    request.port + 1
                },
                ..request
            },
        ] {
            let error = crate::commands::terminate_listener(invalid)
                .await
                .unwrap_err();
            assert_eq!(error.code, CommandErrorCode::ProcessIdentityChanged);
            assert!(
                service.child.try_wait().unwrap().is_none(),
                "negative validation must not signal owned process"
            );
        }
        assert_eq!(
            crate::commands::terminate_listener(TerminateRequest {
                pid: std::process::id(),
                ..request
            })
            .await
            .unwrap_err()
            .code,
            CommandErrorCode::ProcessProtected
        );
        let termination = crate::commands::terminate_listener(request);
        tokio::pin!(termination);
        // Simulate a normal parent reaping its exited child while DevDock observes the PID.
        let result = loop {
            tokio::select! {
                result = &mut termination => break result.unwrap(),
                _ = tokio::time::sleep(Duration::from_millis(10)) => { let _ = service.child.try_wait().unwrap(); }
            }
        };
        assert!(result.signal_sent && result.process_exited);
        assert_eq!(service.child.wait().unwrap().signal(), Some(libc::SIGTERM));
        assert!(
            !crate::commands::list_listeners()
                .await
                .unwrap()
                .iter()
                .any(|row| row.pid == request.pid && row.port == request.port)
        );
        assert_eq!(
            crate::commands::terminate_listener(request)
                .await
                .unwrap_err()
                .code,
            CommandErrorCode::ProcessNotFound
        );
        println!(
            "LIVE PASS: owned pid={} port={}; wrong start/port and self rejected, SIGTERM=15 confirmed, rescan removed listener, vanished PID rejected",
            request.pid, request.port
        );

        let mut unreaped = OwnedService::start(false);
        let request = unreaped.request().await;
        let result = crate::commands::terminate_listener(request).await.unwrap();
        assert!(result.signal_sent && !result.process_exited);
        assert_eq!(unreaped.child.wait().unwrap().signal(), Some(libc::SIGTERM));
        println!(
            "LIVE PASS: owned unreaped pid={} port={}; unreadable zombie metadata returned unconfirmed exit rather than false success",
            request.pid, request.port
        );

        let mut delayed = OwnedService::start(true);
        let request = delayed.request().await;
        let started = std::time::Instant::now();
        let result = crate::commands::terminate_listener(request).await.unwrap();
        assert!(result.signal_sent && !result.process_exited);
        assert!(
            started.elapsed() >= Duration::from_millis(1900)
                && started.elapsed() < Duration::from_secs(3)
        );
        assert!(delayed.child.try_wait().unwrap().is_none());
        assert!(delayed.child.wait().unwrap().success());
        println!(
            "LIVE PASS: owned delayed pid={} port={}; 2-second observation returned signalSent=true/processExited=false, no escalation, process exited on its own timer",
            request.pid, request.port
        );
    }
}

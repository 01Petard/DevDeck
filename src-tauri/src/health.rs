use crate::contracts::{CommandError, CommandErrorCode, HealthCheckResult, HealthStatus};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;

static CLIENT: LazyLock<Result<reqwest::Client, reqwest::Error>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .timeout(Duration::from_secs(3))
        .resolve_to_addrs(
            "localhost",
            &[
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
            ],
        )
        .build()
});
static PERMITS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
static ACTIVE: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

struct ActiveUrl(String);
impl Drop for ActiveUrl {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.lock() {
            active.remove(&self.0);
        }
    }
}

pub(crate) fn validate_url(raw: &str) -> Result<Url, CommandError> {
    let rejected = || CommandError {
        code: CommandErrorCode::HttpUrlNotAllowed,
        message: "仅支持不含用户信息或片段的本机回环 HTTP(S) URL。".into(),
    };
    if raw.chars().count() > 2048
        || raw.chars().any(|c| c.is_whitespace() || c.is_control())
        || raw.contains('\\')
    {
        return Err(rejected());
    }
    let url = Url::parse(raw).map_err(|_| rejected())?;
    let authority = raw
        .split_once("://")
        .ok_or_else(rejected)?
        .1
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    let allowed_authority = ["localhost", "127.0.0.1", "[::1]"].iter().any(|host| {
        authority.eq_ignore_ascii_case(host)
            || authority
                .to_ascii_lowercase()
                .strip_prefix(&format!("{host}:"))
                .is_some_and(|port| !port.is_empty() && port.bytes().all(|c| c.is_ascii_digit()))
    });
    if !allowed_authority
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        || url.port_or_known_default().is_none_or(|port| port == 0)
    {
        return Err(rejected());
    }
    Ok(url)
}

pub async fn check_http_health(raw: String) -> Result<HealthCheckResult, CommandError> {
    let url = validate_url(&raw)?;
    let internal_error = || CommandError {
        code: CommandErrorCode::InternalError,
        message: "暂时无法执行健康检查。".into(),
    };
    {
        let mut active = ACTIVE.lock().map_err(|_| internal_error())?;
        if !active.insert(url.to_string()) {
            return Err(CommandError {
                code: CommandErrorCode::InvalidArgument,
                message: "该 URL 正在检测，请等待完成。".into(),
            });
        }
    }
    let _active = ActiveUrl(url.to_string());
    let _permit = PERMITS.acquire().await.map_err(|_| internal_error())?;
    let client = CLIENT.as_ref().map_err(|_| internal_error())?;
    let started = Instant::now();
    let (status, status_code, message) = match client.get(url).send().await {
        Ok(response) => {
            let code = response.status();
            (
                if code.is_success() {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::HttpError
                },
                Some(code.as_u16()),
                None,
            )
        }
        Err(error) if error.is_timeout() => (
            HealthStatus::Timeout,
            None,
            Some("请求超过 3 秒，服务未及时响应。".into()),
        ),
        Err(_) => (
            HealthStatus::NetworkError,
            None,
            Some("无法连接服务或建立安全连接。".into()),
        ),
    };
    Ok(HealthCheckResult {
        url: raw,
        status,
        status_code,
        latency_ms: Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
        checked_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_allow_only_exact_loopback_hosts_without_credentials_or_fragments() {
        for raw in [
            "http://localhost:8080/health",
            "https://127.0.0.1/",
            "http://[::1]:5173/",
        ] {
            assert!(validate_url(raw).is_ok(), "{raw}");
        }
        for raw in [
            "https://example.com",
            "http://localhost.evil.com",
            "http://localhost.",
            "http://127.0.0.2",
            "http://127.1",
            "http://2130706433",
            "http://0x7f000001",
            "http://[::ffff:127.0.0.1]",
            "http://%6cocalhost",
            "file:///etc/passwd",
            "ftp://localhost",
            "http://user:password@localhost",
            "http://@localhost",
            "http://localhost/#",
            "http://localhost/#fragment",
            "http://localhost:0/",
            "http://localhost:65536/",
            "http://localhost\\@evil.com/",
            "http://local\nhost/",
        ] {
            assert!(validate_url(raw).is_err(), "must reject {raw}");
        }
        assert!(validate_url(&format!("http://localhost/{}", "a".repeat(2048))).is_err());
    }

    async fn server(
        response: &'static str,
        delay: std::time::Duration,
    ) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 4096];
            let count = socket.read(&mut buffer).await.unwrap();
            assert!(buffer[..count].starts_with(b"GET /"));
            let headers = String::from_utf8_lossy(&buffer[..count]).to_lowercase();
            assert!(!headers.contains("authorization:") && !headers.contains("cookie:"));
            tokio::time::sleep(delay).await;
            let _ = socket.write_all(response.as_bytes()).await;
        });
        (format!("http://localhost:{port}/health"), task)
    }

    #[tokio::test]
    async fn http_statuses_resolve_without_redirects_or_body_downloads() {
        for (response, status, code) in [
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\nConnection: close\r\n\r\n",
                HealthStatus::Healthy,
                200,
            ),
            (
                "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n",
                HealthStatus::Healthy,
                204,
            ),
            (
                "HTTP/1.1 301 Moved\r\nLocation: http://192.0.2.1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                HealthStatus::HttpError,
                301,
            ),
            (
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                HealthStatus::HttpError,
                404,
            ),
            (
                "HTTP/1.1 500 Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                HealthStatus::HttpError,
                500,
            ),
        ] {
            let (url, task) = server(response, std::time::Duration::ZERO).await;
            let result = check_http_health(url).await.unwrap();
            assert_eq!(result.status, status);
            assert_eq!(result.status_code, Some(code));
            assert!(result.latency_ms.is_some());
            assert!(result.checked_at > 0);
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn connection_refusal_and_timeout_are_diagnostic_results_and_redacted() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let refused = check_http_health(format!("http://127.0.0.1:{port}/?secret=value"))
            .await
            .unwrap();
        assert_eq!(refused.status, HealthStatus::NetworkError);
        assert!(!refused.message.unwrap_or_default().contains("secret"));
        let (url, task) =
            server("HTTP/1.1 200 OK\r\n\r\n", std::time::Duration::from_secs(5)).await;
        let started = std::time::Instant::now();
        let result = check_http_health(url).await.unwrap();
        assert_eq!(result.status, HealthStatus::Timeout);
        assert!(started.elapsed() >= std::time::Duration::from_millis(2900));
        assert!(started.elapsed() < std::time::Duration::from_secs(4));
        task.abort();
    }

    #[tokio::test]
    async fn concurrent_requests_are_limited_and_duplicate_urls_rejected() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (entered, mut entries) = tokio::sync::mpsc::unbounded_channel();
        let maximum = peak.clone();
        let server = tokio::spawn(async move {
            let mut handlers = Vec::new();
            for _ in 0..8 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let active = active.clone();
                let peak = peak.clone();
                let entered = entered.clone();
                handlers.push(tokio::spawn(async move {
                    let mut bytes = [0; 4096];
                    socket.read(&mut bytes).await.unwrap();
                    let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(count, Ordering::SeqCst);
                    entered.send(()).unwrap();
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    socket
                        .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                        .await
                        .unwrap();
                    active.fetch_sub(1, Ordering::SeqCst);
                }));
            }
            for task in handlers {
                task.await.unwrap();
            }
        });
        let first_url = format!("http://127.0.0.1:{port}/0");
        let first = tokio::spawn(check_http_health(first_url.clone()));
        entries.recv().await.unwrap();
        assert_eq!(
            check_http_health(first_url).await.unwrap_err().code,
            CommandErrorCode::InvalidArgument
        );
        let mut jobs = vec![first];
        for index in 1..8 {
            jobs.push(tokio::spawn(check_http_health(format!(
                "http://127.0.0.1:{port}/{index}"
            ))));
        }
        for job in jobs {
            assert_eq!(job.await.unwrap().unwrap().status, HealthStatus::Healthy);
        }
        server.await.unwrap();
        assert!(maximum.load(Ordering::SeqCst) <= 4);
        assert!(maximum.load(Ordering::SeqCst) > 1);
    }

    #[tokio::test]
    async fn ipv6_loopback_and_tls_failure_are_supported() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.read(&mut [0; 4096]).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        assert_eq!(
            check_http_health(format!("http://[::1]:{port}/"))
                .await
                .unwrap()
                .status,
            HealthStatus::Healthy
        );
        task.await.unwrap();
        // Plain HTTP deliberately receives a TLS handshake; it cannot become healthy.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let tls_server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.read(&mut [0; 4096]).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")
                .await
                .unwrap();
        });
        let result = check_http_health(format!("https://127.0.0.1:{port}/"))
            .await
            .unwrap();
        assert_eq!(result.status, HealthStatus::NetworkError);
        assert_eq!(result.status_code, None);
        tls_server.await.unwrap();
    }

    #[tokio::test]
    async fn configured_proxies_are_bypassed_in_an_isolated_process() {
        let (url, server) = server(
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            Duration::ZERO,
        )
        .await;
        let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
        let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "health::tests::proxy_environment_probe",
            ])
            .env("DEVDOCK_TEST_HEALTH_URL", url)
            .env("NO_PROXY", "")
            .env("no_proxy", "");
        for key in [
            "HTTP_PROXY",
            "http_proxy",
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
        ] {
            command.env(key, &proxy_url);
        }
        let output = tokio::time::timeout(Duration::from_secs(8), command.output())
            .await
            .unwrap()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        server.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(50), proxy.accept())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    #[ignore = "isolated helper invoked by proxy bypass test"]
    async fn proxy_environment_probe() {
        let result = check_http_health(std::env::var("DEVDOCK_TEST_HEALTH_URL").unwrap())
            .await
            .unwrap();
        assert_eq!(result.status, HealthStatus::Healthy);
    }
}

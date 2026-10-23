use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortListener {
    pub pid: u32,
    pub process_name: String,
    pub process_start_time: Option<u64>,
    pub executable_path: Option<String>,
    pub protocol: &'static str,
    pub address: String,
    pub port: u16,
    pub ip_version: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandErrorCode {
    InvalidArgument,
    HttpUrlNotAllowed,
    ScanFailed,
    ScanTimeout,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    HttpError,
    NetworkError,
    Timeout,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResult {
    pub url: String,
    pub status: HealthStatus,
    pub status_code: Option<u16>,
    pub latency_ms: Option<u64>,
    pub checked_at: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
}

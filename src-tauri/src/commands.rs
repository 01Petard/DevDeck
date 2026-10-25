use crate::contracts::{
    CommandError, HealthCheckResult, PortListener, TerminateRequest, TerminateResult,
};

#[tauri::command]
pub async fn list_listeners() -> Result<Vec<PortListener>, CommandError> {
    crate::ports::scan_listeners().await
}

#[tauri::command]
pub async fn check_http_health(url: String) -> Result<HealthCheckResult, CommandError> {
    crate::health::check_http_health(url).await
}

#[tauri::command]
pub async fn terminate_listener(
    request: TerminateRequest,
) -> Result<TerminateResult, CommandError> {
    crate::processes::terminate_listener(request).await
}

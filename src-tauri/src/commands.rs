use crate::contracts::{CommandError, PortListener};

#[tauri::command]
pub async fn list_listeners() -> Result<Vec<PortListener>, CommandError> {
    crate::ports::scan_listeners().await
}

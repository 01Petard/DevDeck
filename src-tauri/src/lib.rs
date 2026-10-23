mod commands;
mod contracts;
mod ports;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::list_listeners])
        .run(tauri::generate_context!())
        .expect("无法启动 DevDock");
}

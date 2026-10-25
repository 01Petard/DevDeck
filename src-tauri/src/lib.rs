mod commands;
mod contracts;
mod health;
mod ports;
mod processes;
mod settings;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_store::Builder::default()
                .default_serialize_fn(settings::serialize_settings)
                .build(),
        )
        .setup(|app| {
            let values =
                settings::read_initial_store(&app.path().app_data_dir()?.join("devdock.json"));
            let store = app
                .store_builder("devdock.json")
                .disable_auto_save()
                .create_new()
                .build()?;
            for (key, value) in values {
                store.set(key, value);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_listeners,
            commands::check_http_health,
            commands::terminate_listener
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 DevDock");
}

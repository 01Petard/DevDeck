mod commands;
mod contracts;
mod health;
mod ports;
mod processes;
mod settings;
use tauri::{Manager, WindowEvent};
#[cfg(target_os = "macos")]
use tauri::RunEvent;
use tauri_plugin_store::StoreExt;

pub fn run() {
    let app = tauri::Builder::default()
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

            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_listeners,
            commands::check_http_health,
            commands::terminate_listener
        ])
        .build(tauri::generate_context!())
        .expect("无法构建 DevDock");
    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = event
        {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

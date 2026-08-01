//! ProxyPanel Client 桌面壳入口（Tauri 2）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let data_dir = state::AppState::default_data_dir();
    tauri::Builder::default()
        .manage(state::AppState::new(data_dir))
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::start_proxy,
            commands::stop_proxy,
            commands::proxy_status,
            commands::list_traffic,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

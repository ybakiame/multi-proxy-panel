//! ProxyPanel Client 桌面壳入口（Tauri 2）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

fn main() {
    // WebKitGTK 在 WSLg / 部分 Linux GPU 环境下启用 DMA-BUF 渲染会导致
    // WebView 黑屏（Tauri on Linux 的社区标准 workaround）。必须在 WebView
    // 初始化前注入；若用户已显式设置该变量则保留用户值。
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let data_dir = state::AppState::default_data_dir();
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(state::AppState::new(data_dir))
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::start_proxy,
            commands::stop_proxy,
            commands::proxy_status,
            commands::list_traffic,
            commands::list_remotes,
            commands::add_remote,
            commands::detect_remote,
            commands::remove_remote,
            commands::fetch_remotes,
            commands::list_tasks,
            commands::run_task,
            commands::import_config,
            commands::list_profiles,
            commands::create_profile,
            commands::get_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::set_profile_enabled,
            commands::preview_core_config,
            commands::list_subscriptions,
            commands::add_subscription,
            commands::remove_subscription,
            commands::set_subscription_enabled,
            commands::refresh_subscription,
            commands::list_cores,
            commands::list_remote_core_versions,
            commands::download_core,
            commands::set_active_core,
            commands::detect_system_cores,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

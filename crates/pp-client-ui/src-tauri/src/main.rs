//! ProxyPanel Client 桌面壳入口（Tauri 2）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

/// WSL 软渲染下 WebKitGTK 黑屏兼容。
///
/// WSL2 中运行 `LIBGL_ALWAYS_SOFTWARE=1` 时，Tauri v2 在 Linux 使用的 WebKitGTK
/// 合成器（compositing）/ DMA-BUF 渲染路径会输出黑屏，这是上游已知问题，与前端
/// 代码无关。通过注入以下环境变量可强制 WebKitGTK 走软件合成路径：
///
/// - `WEBKIT_DISABLE_DMABUF_RENDERER=1`：禁用 DMA-BUF 渲染器。
///
/// 注：实测 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 在部分 WSLg/WebKitGTK 组合下反而
/// 会导致页面完全不渲染，故本函数仅注入 DMA-BUF 禁用；需要禁用合成模式的用户可
/// 自行显式设置该变量。
///
/// 仅在 Linux 且检测到 WSL 内核（`/proc/sys/kernel/osrelease` 内容忽略大小写包含
/// `microsoft` 或 `wsl`）时注入；读取失败视为非 WSL，不注入。若用户已显式设置
/// 同名环境变量，则以用户设置为准，本函数仅在变量未设置时注入，已设置的同名
/// 环境变量优先。
///
/// 必须在任何 WebKit 相关初始化（Tauri 应用构建）之前调用。
#[cfg(target_os = "linux")]
fn configure_wsl_webkit_workaround() {
    let is_wsl = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|osrelease| {
            let osrelease = osrelease.to_lowercase();
            osrelease.contains("microsoft") || osrelease.contains("wsl")
        })
        .unwrap_or(false);

    if !is_wsl {
        return;
    }

    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    // tracing 尚未初始化（见下方 subscriber 的创建），此处用 eprintln! 输出。
    eprintln!(
        "[pp-client-ui] WSL 检测到，已启用 WebKitGTK 兼容模式 \
         (WEBKIT_DISABLE_DMABUF_RENDERER=1)"
    );
}

fn main() {
    // 必须在任何 WebKit 相关初始化之前执行。
    #[cfg(target_os = "linux")]
    configure_wsl_webkit_workaround();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let data_dir = state::AppState::default_data_dir();
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::new(data_dir))
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::start_proxy,
            commands::stop_proxy,
            commands::tun_auth_status,
            commands::authorize_tun,
            commands::proxy_status,
            commands::set_rule_mode,
            commands::list_traffic,
            commands::get_mitm_ca,
            commands::list_remotes,
            commands::add_remote,
            commands::update_remote,
            commands::get_remote_icon,
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
            commands::preview_core_config,
            commands::list_subscriptions,
            commands::add_subscription,
            commands::update_subscription,
            commands::remove_subscription,
            commands::set_subscription_enabled,
            commands::set_active_subscription,
            commands::refresh_subscription,
            commands::list_cores,
            commands::list_remote_core_versions,
            commands::list_downloaded_versions,
            commands::download_core,
            commands::set_active_core,
            commands::detect_system_cores,
            commands::delete_core,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

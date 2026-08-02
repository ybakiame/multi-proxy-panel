//! ProxyPanel Client 桌面壳入口（Tauri 2）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

/// WSL 软渲染下 WebKitGTK 黑屏兼容。
///
/// WSL2 中 Tauri v2 在 Linux 使用的 WebKitGTK 存在两类导致页面全黑的上游已知
/// 问题（与前端代码无关）：
///
/// 1. DMA-BUF 渲染路径在软渲染（如 `LIBGL_ALWAYS_SOFTWARE=1`）下输出黑屏，
///    通过 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 禁用；
/// 2. bubblewrap 沙箱在 WSL 中无法建立（WebKitGTK 2.52.5 实测），Web 内容进程
///    启动即崩溃（日志 `NeedDebuggerBreak trap`、渲染的 `#root` 为空），页面全黑，
///    通过 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` 禁用沙箱规避。
///
/// 安全性说明：`WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS` 变量名本身即含
/// DANGEROUS 警示——禁用后 Web 进程不再受 bubblewrap 沙箱隔离。本应用仅加载本地
/// 打包的前端内容，不加载远程网页，故此处风险可接受。
///
/// 注：实测 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 在部分 WSLg/WebKitGTK 组合下反而
/// 会导致页面完全不渲染，故本函数不注入该变量；需要禁用合成模式的用户可自行
/// 显式设置。
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

    for (name, value) in [
        ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        ("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1"),
    ] {
        if std::env::var_os(name).is_none() {
            std::env::set_var(name, value);
        }
    }

    // tracing 尚未初始化（见下方 subscriber 的创建），此处用 eprintln! 输出。
    eprintln!(
        "[pp-client-ui] WSL 检测到，已启用 WebKitGTK 兼容模式 \
         (WEBKIT_DISABLE_DMABUF_RENDERER=1, WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1)"
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

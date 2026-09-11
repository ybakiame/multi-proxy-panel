//! ProxyPanel Client 移动壳（Tauri 2，Android 目标）。
//!
//! 命令层全部来自共享 crate [`pp-client-tauri`]（双端通用命令、日志系统、平台能力
//! 矩阵与共享状态，见 ADR-0003 §3.2/§3.3），本壳只做平台组装：
//!
//! - 数据目录：Android 应用私有目录（`app_data_dir()`，HOME 在 Android 为只读 `/`）；
//! - 日志：初始化共享日志系统并把保活 guard 存入 [`AppState`]；
//! - 插件：通知（`tauri-plugin-notification`）与 opener（`tauri-plugin-opener`）；
//! - 命令：按全路径注册共享 crate 的全部命令（35 条 commands + 2 capabilities +
//!   7 logs + Android 专属三命令，M3.5 引入），不注册 desktop 专属（MITM/核心管理/
//!   远程资源等）。
//!
//! 同时作为 lib 与 bin 构建：Android/iOS 端由 [`tauri::mobile_entry_point`] 注入移动
//! 入口（`main` 不参与编译）；host（桌面）构建时由 `main` 调用 [`run`]。
//!
//! 与 desktop 壳的差异即 ADR-0003 收益：无 WSL workaround（桌面专属）、无本地命令
//! 模块、无 `pp-mitm`/`pp-script` 直接依赖（见 `Cargo.toml` 注释）。

use std::path::PathBuf;

use tauri::Manager;

/// 解析应用数据目录（Android 语义，对齐 desktop 壳 `lib.rs` 的 Android 分支）。
///
/// Android 上 `HOME` 为只读 `/`，写 `$HOME/.proxy-panel-client` 必然失败；此处统一
/// 走 Tauri path resolver 的应用私有可写目录 `app_data_dir()`（如
/// `/data/user/0/com.proxypanel.client/files`，随应用卸载清除，属移动端预期语义），
/// 不可用时回退 `app_local_data_dir()`，仍失败视为配置错误，终止启动并报错。
///
/// 不提供 desktop 分支：移动壳 host 构建仅供编译检查，运行目标是 Android。
fn resolve_data_dir(app: &tauri::App) -> PathBuf {
    match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(app_data_err) => {
            // tracing 尚未初始化（见 `run()`：日志初始化紧随本函数），此处的
            // 告警用 eprintln 直出 stderr，保证诊断可见。
            eprintln!(
                "[pp-client-mobile-ui] Android app_data_dir() 解析失败：{app_data_err}，回退 app_local_data_dir()"
            );
            match app.path().app_local_data_dir() {
                Ok(dir) => dir,
                Err(local_data_err) => panic!(
                    "Android 应用数据目录解析失败：app_data_dir {app_data_err}，app_local_data_dir {local_data_err}"
                ),
            }
        }
    }
}

/// 应用入口。
///
/// Android/iOS 构建时经 `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 生成
/// 移动端 JNI/入口；host（桌面）构建时由 `main` 调用本函数。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Android 上 HOME 为只读 `/`，数据目录须在 setup 阶段经 AppHandle
            // 的 path resolver 解析应用私有目录。
            let data_dir = resolve_data_dir(app);
            // tracing 全局 subscriber 只能初始化一次；日志初始化紧随 data_dir
            // 解析，guard 存入 AppState 持有，保证进程生命周期内文件写入线程存活。
            let log_guard = pp_client_tauri::logs::init_logging(&data_dir);
            tracing::info!("ProxyPanel 客户端数据目录：{}", data_dir.display());
            app.manage(pp_client_tauri::state::AppState::new(data_dir, log_guard));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pp_client_tauri::capabilities::get_capabilities,
            pp_client_tauri::capabilities::platform_info,
            pp_client_tauri::logs::get_logs,
            pp_client_tauri::logs::export_logs,
            pp_client_tauri::logs::open_export_dir,
            pp_client_tauri::logs::list_log_files,
            pp_client_tauri::logs::read_log_file_tail,
            pp_client_tauri::logs::clear_logs,
            pp_client_tauri::logs::log_frontend,
            pp_client_tauri::commands::get_config,
            pp_client_tauri::commands::save_config,
            pp_client_tauri::commands::start_proxy,
            pp_client_tauri::commands::stop_proxy,
            pp_client_tauri::commands::proxy_status,
            pp_client_tauri::commands::set_rule_mode,
            pp_client_tauri::commands::list_profiles,
            pp_client_tauri::commands::create_profile,
            pp_client_tauri::commands::get_profile,
            pp_client_tauri::commands::update_profile,
            pp_client_tauri::commands::delete_profile,
            pp_client_tauri::commands::preview_core_config,
            pp_client_tauri::commands::list_subscriptions,
            pp_client_tauri::commands::add_subscription,
            pp_client_tauri::commands::update_subscription,
            pp_client_tauri::commands::remove_subscription,
            pp_client_tauri::commands::set_subscription_enabled,
            pp_client_tauri::commands::set_active_subscription,
            pp_client_tauri::commands::refresh_subscription,
            pp_client_tauri::commands::subscription_node_tags,
            pp_client_tauri::commands::proxies_list,
            pp_client_tauri::commands::proxies_select,
            pp_client_tauri::commands::proxies_test_delay,
            pp_client_tauri::commands::proxies_test_group,
            pp_client_tauri::commands::connections_active,
            pp_client_tauri::commands::connections_closed,
            pp_client_tauri::commands::connections_close,
            pp_client_tauri::commands::list_tasks,
            pp_client_tauri::commands::run_task,
            pp_client_tauri::commands::local_override_get,
            pp_client_tauri::commands::local_override_save,
            pp_client_tauri::commands::local_override_update_rulesets_now,
            pp_client_tauri::commands::local_override_update_rule_set,
            pp_client_tauri::commands::config_slices_get,
            pp_client_tauri::commands::config_slices_save,
            // Android 专属三命令来自共享层 pp-client-tauri::core_bridge（ADR-0003
            // M3.5）。mobile 壳只编 android target，host 构建仅供编译检查，模块在
            // host 不可见，故以 cfg 显式包裹（桌面构建裁剪）。
            #[cfg(target_os = "android")]
            pp_client_tauri::core_bridge::request_vpn_permission,
            #[cfg(target_os = "android")]
            pp_client_tauri::core_bridge::vpn_last_error,
            #[cfg(target_os = "android")]
            pp_client_tauri::core_bridge::core_version,
            #[cfg(target_os = "android")]
            pp_client_tauri::core_bridge::notify_prefs_changed,
        ]);

    // Android 核心由 Kotlin 侧 libbox 驱动：注册 `vpn` 插件（VpnPlugin 真实桥，setup
    // 内注册 Kotlin 插件并安装核心引擎桥，见 pp-client-tauri::core_bridge::vpn_plugin）。
    #[cfg(target_os = "android")]
    let builder = builder.plugin(pp_client_tauri::core_bridge::vpn_plugin());

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

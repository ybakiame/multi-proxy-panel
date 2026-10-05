//! ProxyPanel 客户端壳（Tauri 2，单壳双目标：Linux/Windows/macOS + Android）。
//!
//! 合并架构（docs/plans/2026-10-03-client-merge-evaluation.md，M1）：双端通用命令
//! 单份实现于共享 crate [`pp_client_tauri`]（35+ 条 commands + capabilities + logs），
//! 本壳只做平台组装，平台差异为编译期事实：
//!
//! - [`desktop`] 适配层（`#[cfg(not(any(android, ios)))]`）：桌面专属命令
//!   （mitm / core_mgmt / remote / platform）、数据目录与 WSL workaround；
//! - [`mobile`] 适配层（`#[cfg(target_os = "android")]`）：Android 应用私有数据
//!   目录、`core_bridge` 数据目录注入与 VPN 插件注册；
//! - 依赖差异由 `Cargo.toml` target 依赖表表达：Android 构建图不含 `pp-mitm`
//!   （`pp-client-tauri/mitm` feature 仅桌面目标激活，M0-① 实证）。
//!
//! 同时作为 lib 与 bin 构建：Android/iOS 端由 [`tauri::mobile_entry_point`] 注入
//! 移动入口（`main` 不参与编译）；桌面端由 `main` 调用 [`run()`]。

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop;
#[cfg(target_os = "android")]
mod mobile;

use tauri::Manager;

/// 应用入口。
///
/// Android/iOS 构建时经 `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 生成
/// 移动端 JNI/入口；桌面构建时由 `main` 调用本函数。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 必须在任何 WebKit 相关初始化之前执行（桌面 Linux 专属；target_os = "linux"
    // 不含 Android，无需额外排除）。
    #[cfg(all(target_os = "linux", not(any(target_os = "android", target_os = "ios"))))]
    desktop::configure_wsl_webkit_workaround();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init());

    // 自动更新（ADR-0008 D2）：仅桌面目标注册（Android 不经 updater 分发，
    // 对应 capability 亦按 platforms 门控于桌面）。
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    let builder = builder
        .setup(|app| {
            // 数据目录：桌面 `$HOME/.proxy-panel-client`；Android 应用私有目录
            // （HOME 在 Android 为只读 `/`）。
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let data_dir = desktop::default_data_dir();
            #[cfg(target_os = "android")]
            let data_dir = mobile::resolve_data_dir(app);
            // tracing 全局 subscriber 只能初始化一次；日志初始化紧随 data_dir
            // 解析，guard 存入 AppState 持有，保证进程生命周期内文件写入线程存活。
            let log_guard = pp_client_tauri::logs::init_logging(&data_dir);
            tracing::info!("ProxyPanel 客户端数据目录：{}", data_dir.display());
            app.manage(pp_client_tauri::state::AppState::new(data_dir.clone(), log_guard));
            // 安装包内置种子核心首启释放（ADR-0008 D5）：仅桌面目标；无种子资源
            // （dev / Linux / macOS 构建）或已装核心时静默跳过，失败仅告警不阻塞启动。
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if let Ok(resource_dir) = app.path().resource_dir() {
                match pp_client::cores::seed_bundled_core(&data_dir, &resource_dir) {
                    Ok(Some(core)) => {
                        tracing::info!(version = %core.version, "种子核心已释放并就绪");
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "种子核心释放失败，可在核心管理中手动下载");
                    }
                }
            }
            // 向 Android 核心桥注入数据目录（VPN 通知偏好/订阅名解析回源
            // client.json / subscriptions.json）。
            #[cfg(target_os = "android")]
            pp_client_tauri::core_bridge::set_data_dir(data_dir.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ---- 双端通用命令（共享层单份实现，全路径注册）----
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
            pp_client_tauri::commands::stats_today,
            pp_client_tauri::commands::stats_daily,
            pp_client_tauri::commands::stats_records,
            pp_client_tauri::commands::stats_clear,
            pp_client_tauri::commands::list_tasks,
            pp_client_tauri::commands::run_task,
            pp_client_tauri::commands::local_override_get,
            pp_client_tauri::commands::local_override_save,
            pp_client_tauri::commands::local_override_update_rulesets_now,
            pp_client_tauri::commands::local_override_update_rule_set,
            pp_client_tauri::commands::config_slices_get,
            pp_client_tauri::commands::config_slices_save,
            pp_client_tauri::commands::baseline_view_get,
            pp_client_tauri::commands::builtin_dns_slice_get,
            pp_client_tauri::commands::diagnose_connectivity_run,
            pp_client_tauri::commands::dns_server_probe,
            // ---- 桌面专属命令（desktop 适配层；移动端编译期裁剪）----
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::tun_auth_status,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::authorize_tun,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::list_traffic,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::get_mitm_ca,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::list_remotes,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::add_remote,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::update_remote,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::get_remote_icon,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::detect_remote,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::remove_remote,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::fetch_remotes,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::test_github_proxy,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::import_config,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::list_cores,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::list_remote_core_channels,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::list_downloaded_versions,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::download_core,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::set_active_core,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::detect_system_cores,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::delete_core,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::gpu_acceleration,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            desktop::commands::toast_mode_override,
            // ---- Android 专属命令（共享层 core_bridge；桌面编译期裁剪）----
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

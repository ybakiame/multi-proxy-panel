//! ProxyPanel Client 桌面壳（Tauri 2 命令层）。
//!
//! 同时作为 lib 与 bin 构建：桌面端 `main` 调用 [`run`]；Android/iOS 端由
//! [`tauri::mobile_entry_point`] 注入移动入口，`main` 不参与编译。

pub mod commands;
#[cfg(target_os = "android")]
mod core_bridge;
pub mod logs;
mod state;

use tauri::Manager;

/// WSL 下 WebKitGTK 兼容与 GPU 处理。
///
/// WSL2 中 Tauri v2 在 Linux 使用的 WebKitGTK 存在导致页面全黑的上游已知问题
/// （与前端代码无关）：
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
/// GPU 三级策略：
/// - 有 WSLg GPU 直通（`/dev/dxg` 存在）时走硬件加速，不注入
///   `LIBGL_ALWAYS_SOFTWARE`；注意 GPU 直通可用不代表渲染必然成功，此处只是
///   不主动降级；
/// - 无 GPU 直通（`/dev/dxg` 不存在）时，Mesa 硬件探测会输出一串 libEGL/ZINK
///   失败警告后回退 llvmpipe，此时自动注入 `LIBGL_ALWAYS_SOFTWARE=1` 直接走软渲染，
///   跳过无谓的探测；
/// - 用户显式设置的环境变量始终优先，本函数仅在对应变量未设置时注入。
///
/// 判断 `/proc/sys/kernel/osrelease` 内容是否为 WSL 内核标识。
///
/// 规则：忽略大小写后包含 `microsoft` 或 `wsl` 即视为 WSL。`lib.rs`
/// （WebKit 兼容注入）与 `commands.rs`（`gpu_acceleration` 检测）共用，避免两处
/// 实现漂移。
pub(crate) fn is_wsl_osrelease(osrelease: &str) -> bool {
    let osrelease = osrelease.to_lowercase();
    osrelease.contains("microsoft") || osrelease.contains("wsl")
}

/// 是否运行在 WSL（Linux 子系统）中。
///
/// 依据 `/proc/sys/kernel/osrelease`（见 [`is_wsl_osrelease`]）；非 Linux 平台或
/// 读取失败时返回 `false`。
pub(crate) fn is_wsl() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|osrelease| is_wsl_osrelease(&osrelease))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// 仅在 Linux 且检测到 WSL 内核（`/proc/sys/kernel/osrelease` 内容忽略大小写包含
/// `microsoft` 或 `wsl`，见 [`is_wsl`]）时生效；读取失败视为非 WSL，不注入。
///
/// 必须在任何 WebKit 相关初始化（Tauri 应用构建）之前调用。
#[cfg(target_os = "linux")]
fn configure_wsl_webkit_workaround() {
    if !is_wsl() {
        return;
    }

    // 实际注入的变量列表（用户已显式设置的不会注入），日志按实际注入输出。
    let mut injected: Vec<&str> = Vec::new();

    for (name, value) in [
        ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        ("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS", "1"),
    ] {
        if std::env::var_os(name).is_none() {
            std::env::set_var(name, value);
            injected.push(name);
        }
    }

    // WSLg GPU 半虚拟化设备：/dev/dxg 不存在即无硬件 GPU 直通，Mesa 硬件探测会
    // 输出一串 libEGL/ZINK 失败警告后回退 llvmpipe。自动注入
    // LIBGL_ALWAYS_SOFTWARE=1 直接走软渲染，跳过无谓的探测。
    let has_dxg = std::path::Path::new("/dev/dxg").exists();
    if !has_dxg && std::env::var_os("LIBGL_ALWAYS_SOFTWARE").is_none() {
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        injected.push("LIBGL_ALWAYS_SOFTWARE");
    }

    // tracing 尚未初始化（见下方 subscriber 的创建），此处用 eprintln! 输出。
    let injected_log = if injected.is_empty() {
        "无（均由用户显式设置）".to_string()
    } else {
        injected.join(", ")
    };
    eprintln!(
        "[pp-client-ui] WSL 检测到，GPU 直通{}，已注入：{}",
        if has_dxg {
            "可用"
        } else {
            "不可用（自动软渲染）"
        },
        injected_log,
    );
}

/// 解析应用数据目录。
///
/// - 桌面：沿用 [`state::AppState::default_data_dir`]（`$HOME/.proxy-panel-client`），
///   行为与既有版本完全一致，不迁移已有数据；
/// - Android：`HOME` 为只读 `/`，写 `$HOME/.proxy-panel-client` 必然失败；改用
///   Tauri path resolver 的应用私有可写目录 `app_data_dir()`（如
///   `/data/user/0/com.proxypanel.client/files`，随应用卸载清除，属移动端预期语义），
///   不可用时回退 `app_local_data_dir()`，仍失败视为配置错误，终止启动并报错。
fn resolve_data_dir(app: &tauri::App) -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    {
        match app.path().app_data_dir() {
            Ok(dir) => return dir,
            Err(app_data_err) => {
                // tracing 尚未初始化（见 `run()`：日志初始化紧随本函数），此处的
                // 告警用 eprintln 直出 stderr，保证诊断可见。
                eprintln!(
                    "[pp-client-ui] Android app_data_dir() 解析失败：{app_data_err}，回退 app_local_data_dir()"
                );
                match app.path().app_local_data_dir() {
                    Ok(dir) => return dir,
                    Err(local_data_err) => panic!(
                        "Android 应用数据目录解析失败：app_data_dir {app_data_err}，app_local_data_dir {local_data_err}"
                    ),
                }
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        state::AppState::default_data_dir()
    }
}

/// 应用入口。
///
/// Android/iOS 构建时经 `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 生成
/// 移动端 JNI/入口；桌面构建时由 `main` 调用本函数。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 必须在任何 WebKit 相关初始化之前执行。
    #[cfg(target_os = "linux")]
    configure_wsl_webkit_workaround();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Android 上 HOME 为只读 `/`，数据目录须在 setup 阶段经 AppHandle
            // 的 path resolver 解析应用私有目录（桌面仍走 default_data_dir）。
            let data_dir = resolve_data_dir(app);
            // tracing 全局 subscriber 只能初始化一次；日志初始化紧随 data_dir
            // 解析，guard 存入 AppState 持有，保证进程生命周期内文件写入线程存活。
            let log_guard = logs::init_logging(&data_dir);
            tracing::info!("ProxyPanel 客户端数据目录：{}", data_dir.display());
            app.manage(state::AppState::new(data_dir, log_guard));
            Ok(())
        })
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
            commands::test_github_proxy,
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
            commands::gpu_acceleration,
            commands::toast_mode_override,
            #[cfg(target_os = "android")]
            commands::request_vpn_permission,
            #[cfg(target_os = "android")]
            commands::vpn_last_error,
            commands::platform_info,
            commands::get_capabilities,
            logs::get_logs,
            logs::export_logs,
            logs::open_export_dir,
            logs::list_log_files,
            logs::read_log_file_tail,
            logs::clear_logs,
            logs::log_frontend,
        ]);

    // Android 核心由 Kotlin 侧 libbox 驱动：`vpn` 插件 setup 中注册 VpnPlugin
    // 并安装真实核心引擎桥（P1c-2）。桌面端不注册插件，行为零变化。
    #[cfg(target_os = "android")]
    let builder = builder.plugin(core_bridge::vpn_plugin());

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

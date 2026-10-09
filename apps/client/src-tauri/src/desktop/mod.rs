//! 桌面适配层（Linux / Windows / macOS）：桌面专属命令、数据目录解析与
//! WSL/WebKitGTK 兼容处理。
//!
//! 仅在非移动目标编译（`lib.rs` 以 `#[cfg(not(any(android, ios)))]` 门控模块
//! 声明）；移动端对应物为 `crate::mobile`。

pub mod commands;
pub mod state;

pub use state::default_data_dir;

/// 退出清理（ADR-0012 D3）：`RunEvent::ExitRequested` 时 best-effort 停止核心
/// 并恢复系统代理（`ClientState::stop` 的完整逆序关闭）。
///
/// 仅覆盖正常退出路径；进程被杀（冻结后结束任务、崩溃）时收不到本事件，由
/// pp-core 的 OS 级父子绑定与 PID 收割兜底。整体 5s 超时保护，任何失败仅
/// 告警，绝不阻塞退出。
pub fn cleanup_on_exit(app: &tauri::AppHandle) {
    use tauri::Manager;
    let state = app.state::<pp_client_tauri::state::AppState>();
    let client = state.client.clone();
    // 退出回调运行在事件循环线程（非 tokio worker），需临时运行时驱动异步清理。
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(runtime) = runtime else {
        eprintln!("[pp-client-app] 退出清理：临时运行时创建失败，跳过");
        return;
    };
    runtime.block_on(async move {
        let cleanup = async {
            let mut lock = client.lock().await;
            if let Some(client) = lock.as_mut() {
                client.stop().await;
            }
        };
        if tokio::time::timeout(std::time::Duration::from_secs(5), cleanup)
            .await
            .is_err()
        {
            tracing::warn!("退出清理超时（5s），照常退出");
        }
    });
}

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
/// 规则：忽略大小写后包含 `microsoft` 或 `wsl` 即视为 WSL。`desktop/mod.rs`
/// （WebKit 兼容注入）与 `desktop/commands/platform.rs`（`gpu_acceleration`
/// 检测）共用，避免两处实现漂移。
pub(crate) fn is_wsl_osrelease(osrelease: &str) -> bool {
    let osrelease = osrelease.to_lowercase();
    osrelease.contains("microsoft") || osrelease.contains("wsl")
}

/// 是否运行在 WSL（Linux 子系统）中。
///
/// 依据 `/proc/sys/kernel/osrelease`（见 [`is_wsl_osrelease`]）；仅 Linux 需要
/// （唯一消费方是 Linux-only 的 [`configure_wsl_webkit_workaround`]）。target_os
/// 语义：`linux` 不含 Android（`target_os = "android"`），故本模块的 linux cfg
/// 不会误命中移动目标。
#[cfg(target_os = "linux")]
pub(crate) fn is_wsl() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|osrelease| is_wsl_osrelease(&osrelease))
        .unwrap_or(false)
}

/// 仅在 Linux 且检测到 WSL 内核（`/proc/sys/kernel/osrelease` 内容忽略大小写包含
/// `microsoft` / `wsl`，见 [`is_wsl`]）时生效；读取失败视为非 WSL，不注入。
///
/// 必须在任何 WebKit 相关初始化（Tauri 应用构建）之前调用。
#[cfg(target_os = "linux")]
pub(crate) fn configure_wsl_webkit_workaround() {
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
    // 输出一串 libEGL/ZINK 失败警告后回退 llvmpipe。自动注入 `LIBGL_ALWAYS_SOFTWARE=1`
    // 直接走软渲染，跳过无谓的探测。
    let has_dxg = std::path::Path::new("/dev/dxg").exists();
    if !has_dxg && std::env::var_os("LIBGL_ALWAYS_SOFTWARE").is_none() {
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        injected.push("LIBGL_ALWAYS_SOFTWARE");
    }

    // tracing 尚未初始化（见 lib.rs `run()`：日志初始化在 setup 阶段），此处用
    // eprintln! 输出。
    let injected_log = if injected.is_empty() {
        "无（均由用户显式设置）".to_string()
    } else {
        injected.join(", ")
    };
    eprintln!(
        "[pp-client-app] WSL 检测到，GPU 直通{}，已注入：{}",
        if has_dxg {
            "可用"
        } else {
            "不可用（自动软渲染）"
        },
        injected_log,
    );
}

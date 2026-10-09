//! 移动适配层（Android）：应用私有数据目录解析。
//!
//! 仅在 Android 目标编译（`lib.rs` 以 `#[cfg(target_os = "android")]` 门控模块
//! 声明）；桌面端对应物为 `crate::desktop`。

use std::path::PathBuf;

use tauri::Manager;

/// 解析应用数据目录（Android 语义）。
///
/// Android 上 `HOME` 为只读 `/`，写 `$HOME/.proxy-panel-client` 必然失败；统一
/// 走 Tauri path resolver 的应用私有可写目录 `app_data_dir()`（如
/// `/data/user/0/com.proxypanel.client/files`，随应用卸载清除，属移动端预期语义），
/// 不可用时回退 `app_local_data_dir()`，仍失败视为配置错误，终止启动并报错。
pub(crate) fn resolve_data_dir(app: &tauri::App) -> PathBuf {
    match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(app_data_err) => {
            // tracing 尚未初始化（见 `lib.rs` `run()`：日志初始化紧随本函数），此处的
            // 告警用 eprintln 直出 stderr，保证诊断可见。
            eprintln!(
                "[pp-client-app] Android app_data_dir() 解析失败：{app_data_err}，回退 app_local_data_dir()"
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

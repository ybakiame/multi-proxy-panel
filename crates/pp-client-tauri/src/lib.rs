//! pp-client-tauri — 双端通用 Tauri 命令层。
//!
//! Desktop / Mobile 壳共享的 Tauri 命令实现与辅助（见 ADR-0003 §3.2）：应用级共享
//! 状态（[`state`]）、日志系统（[`logs`]）、平台能力矩阵（[`capabilities`]）与命令
//! 共享辅助（[`commands`]）。
//!
//! 壳层以路径注册本 crate 的命令（Tauri 2 支持跨 crate 注册，命令名由宏按函数名
//! 生成，与注册路径无关）：
//!
//! ```ignore
//! tauri::generate_handler![
//!     pp_client_tauri::logs::get_logs,
//!     pp_client_tauri::capabilities::get_capabilities,
//!     // ...
//! ]
//! ```

pub mod capabilities;
pub mod commands;
pub mod logs;
pub mod state;

pub use capabilities::*;
pub use logs::*;
pub use state::*;

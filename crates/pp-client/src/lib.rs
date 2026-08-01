//! pp-client — 桌面客户端核心库。
//!
//! 提供客户端配置（[`config`]）、订阅同步（[`subscription`]）、核心配置合成
//! （[`core_config`]）、系统代理（[`sysproxy`]）、核心运行器（[`runner`]）、
//! MITM 构建（[`mitm`]）与运行状态编排（[`state`]）。

#![allow(clippy::result_large_err)]

pub mod config;
pub mod core_config;
pub mod mitm;
pub mod runner;
pub mod state;
pub mod subscription;
pub mod sysproxy;

pub use config::*;
pub use core_config::*;
pub use mitm::*;
pub use runner::*;
pub use state::*;
pub use subscription::*;
pub use sysproxy::*;

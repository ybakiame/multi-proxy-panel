//! pp-client — 桌面客户端核心库。
//!
//! 提供客户端配置（[`config`]）、订阅同步（[`subscription`]）与
//! 核心配置合成（[`core_config`]）。

#![allow(clippy::result_large_err)]

pub mod config;
pub mod core_config;
pub mod subscription;

pub use config::*;
pub use core_config::*;
pub use subscription::*;

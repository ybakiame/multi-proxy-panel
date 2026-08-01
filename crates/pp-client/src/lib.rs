//! pp-client — 桌面客户端核心库。
//!
//! 提供客户端配置（[`config`]）、分享链接解析（[`share_link`]）、双核心节点转换
//! （[`node_convert`]）、通用订阅管理（[`subscription`]）、三方配置片段导入
//! （[`import`]）、Profile 模板与复写（[`profile`]）、核心配置合成（[`core_config`]）、
//! 系统代理（[`sysproxy`]）、核心运行器（[`runner`]）、MITM 构建（[`mitm`]）与运行状态编排（[`state`]）。

#![allow(clippy::result_large_err)]

pub mod config;
pub mod core_config;
pub mod cores;
pub mod http_exec;
pub mod import;
pub mod mitm;
pub mod node_convert;
pub mod profile;
pub mod remote;
pub mod runner;
pub mod share_link;
pub mod state;
pub mod subscription;
pub mod sysproxy;

pub use config::*;
pub use core_config::*;
pub use cores::*;
pub use http_exec::*;
pub use import::*;
pub use mitm::*;
pub use node_convert::*;
pub use profile::*;
pub use remote::*;
pub use runner::*;
pub use share_link::*;
pub use state::*;
pub use subscription::*;
pub use sysproxy::*;

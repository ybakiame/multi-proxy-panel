//! MITM 代理基础模块（ProxyPanel）。
//!
//! 提供中间人代理所需的基础组件：CA 证书管理（[`ca`]）、CA 系统信任库安装与
//! 状态检测（[`ca_trust`]）、主机名匹配与基础配置（[`config`]）、hudsucker
//! 拦截代理（[`proxy`]）、URL/Header/Body 重写引擎（[`rewrite`]）、流量记录
//! （[`recorder`]）以及脚本钩子（[`script_hook`]）。

pub mod ca;
pub mod ca_trust;
pub mod config;
mod intercept;
pub mod proxy;
#[cfg(test)]
mod proxy_tests;
pub mod recorder;
pub mod rewrite;
#[cfg(test)]
mod rewrite_tests;
pub mod script_hook;
pub mod upstream;

pub use ca::*;
pub use ca_trust::*;
pub use config::*;
pub use proxy::*;
pub use recorder::*;
pub use rewrite::*;
pub use script_hook::*;
pub use upstream::*;

//! MITM 代理基础模块（ProxyPanel）。
//!
//! 提供中间人代理所需的基础组件：CA 证书管理（[`ca`]）、主机名匹配与
//! 基础配置（[`config`]）、URL/Header/Body 重写引擎（[`rewrite`]）、流量记录
//! （[`recorder`]）以及脚本钩子（[`script_hook`]）。代理层（proxy）在后续迭代中实现。

#![allow(clippy::result_large_err)]

pub mod ca;
pub mod config;
pub mod recorder;
pub mod script_hook;
pub mod rewrite;

pub use ca::*;
pub use config::*;
pub use recorder::*;
pub use script_hook::*;
pub use rewrite::*;

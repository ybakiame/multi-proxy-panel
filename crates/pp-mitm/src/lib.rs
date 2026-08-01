//! MITM 代理基础模块（ProxyPanel）。
//!
//! 提供中间人代理所需的基础组件：CA 证书管理（[`ca`]）、主机名匹配与
//! 基础配置（[`config`]）、URL/Header/Body 重写引擎（[`rewrite`]）以及
//! 流量记录（[`recorder`]）。脚本钩子与代理层（script_hook / proxy）在后续
//! 迭代中实现。

#![allow(clippy::result_large_err)]

pub mod ca;
pub mod config;
pub mod recorder;
pub mod rewrite;

pub use ca::*;
pub use config::*;
pub use recorder::*;
pub use rewrite::*;

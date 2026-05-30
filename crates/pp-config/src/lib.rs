//! pp-config — Kernel configuration builders for xray-core and sing-box.

#![allow(clippy::result_large_err)]

pub mod builder;
pub mod singbox;
pub mod xray;

pub use builder::*;

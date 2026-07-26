//! pp-config — Kernel configuration builders for sing-box and mihomo.

#![allow(clippy::result_large_err)]

pub mod builder;
pub mod mihomo;
pub mod relay;
pub mod singbox;

pub use builder::*;
pub use relay::*;

//! pp-core — Core process management abstraction for sing-box and mihomo.

#![allow(clippy::result_large_err)]

pub mod core_api;
pub mod installer;
pub mod manager;
pub mod supervisor;

pub use core_api::*;
pub use installer::*;
pub use manager::*;
pub use supervisor::*;

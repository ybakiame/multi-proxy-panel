//! pp-core — Core process management abstraction for sing-box and mihomo.

pub mod installer;
mod lifecycle;
pub mod manager;
pub mod supervisor;

pub use installer::*;
pub use manager::*;
pub use supervisor::*;

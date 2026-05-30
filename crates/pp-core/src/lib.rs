//! pp-core — Core process management abstraction for xray and sing-box.

#![allow(clippy::result_large_err)]

pub mod manager;
pub mod supervisor;
pub mod traffic;

pub use manager::*;
pub use supervisor::*;
pub use traffic::*;

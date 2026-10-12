//! Core process manager trait, factory and shared helpers.
//!
//! 管理器实现按核心拆分为子模块：[`singbox`] / [`mihomo`]。

mod mihomo;
mod path;
mod singbox;

pub use mihomo::*;
pub use singbox::*;

use pp_common::{CoreType, PanelResult};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;

/// Manages the lifecycle of a proxy core (sing-box or mihomo).
#[async_trait::async_trait]
pub trait CoreManager: Send + Sync {
    /// Core type this manager handles.
    fn core_type(&self) -> CoreType;

    /// Start the core with the given configuration.
    async fn start(&self, config: &Value) -> PanelResult<()>;

    /// Stop the running core gracefully.
    async fn stop(&self) -> PanelResult<()>;

    /// Restart the core with a new configuration.
    async fn restart(&self, config: &Value) -> PanelResult<()>;

    /// Check if the core is currently running.
    async fn is_running(&self) -> bool;

    /// Reload configuration without restart (if supported by core).
    async fn reload(&self, config: &Value) -> PanelResult<()>;

    /// Get core version string.
    async fn version(&self) -> PanelResult<String>;

    /// Uptime in seconds if the core is running.
    async fn uptime_secs(&self) -> PanelResult<u64>;

    /// Active inbound tags (if queryable).
    async fn active_inbounds(&self) -> PanelResult<Vec<String>>;

    /// Last recorded error message.
    async fn last_error(&self) -> PanelResult<String>;
}

/// Factory for creating CoreManager instances.
pub struct CoreManagerFactory;

impl CoreManagerFactory {
    pub fn create(
        core_type: CoreType,
        binary_path: impl AsRef<Path>,
        config_dir: impl AsRef<Path>,
    ) -> PanelResult<Box<dyn CoreManager>> {
        match core_type {
            CoreType::SingBox => Ok(Box::new(SingBoxProcessManager::new(
                binary_path,
                config_dir,
            )?)),
            CoreType::Mihomo => Ok(Box::new(MihomoProcessManager::new(
                binary_path,
                config_dir,
            )?)),
        }
    }
}

/// Spawn a task that reads lines from `reader` and keeps the last non-empty
/// line in `last_error`.
fn spawn_output_reader<R>(reader: R, last_error: Arc<RwLock<String>>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let reader = BufReader::new(reader);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                *last_error.write().await = trimmed.to_string();
            }
        }
    });
}

/// If no error has been captured yet, record the process exit reason.
/// Otherwise append the exit code to the existing captured output.
async fn record_exit(last_error: Arc<RwLock<String>>, code: i32) {
    let mut err = last_error.write().await;
    if err.is_empty() {
        *err = format!("process exited with code {}", code);
    } else {
        *err = format!("{} (exit code {})", err, code);
    }
}

/// Return the first non-empty line of multi-line output, trimmed.
fn first_output_line(output: &str) -> String {
    output
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

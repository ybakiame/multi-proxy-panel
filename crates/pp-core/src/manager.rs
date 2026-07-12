use pp_common::{CoreType, PanelError, PanelResult};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Manages the lifecycle of a proxy core (xray or sing-box).
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
            CoreType::Xray => Ok(Box::new(XrayProcessManager::new(binary_path, config_dir)?)),
            CoreType::SingBox => Ok(Box::new(SingBoxProcessManager::new(
                binary_path,
                config_dir,
            )?)),
        }
    }
}

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

/// xray-core process manager.
pub struct XrayProcessManager {
    binary: PathBuf,
    config_dir: PathBuf,
    process: RwLock<Option<Child>>,
    start_time: RwLock<Option<Instant>>,
    last_error: Arc<RwLock<String>>,
}

impl XrayProcessManager {
    pub fn new(binary: impl AsRef<Path>, config_dir: impl AsRef<Path>) -> PanelResult<Self> {
        Ok(Self {
            binary: binary.as_ref().to_path_buf(),
            config_dir: config_dir.as_ref().to_path_buf(),
            process: RwLock::new(None),
            start_time: RwLock::new(None),
            last_error: Arc::new(RwLock::new(String::new())),
        })
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("xray.json")
    }
}

#[async_trait::async_trait]
impl CoreManager for XrayProcessManager {
    fn core_type(&self) -> CoreType {
        CoreType::Xray
    }

    async fn start(&self, config: &Value) -> PanelResult<()> {
        if !tokio::fs::try_exists(&self.binary).await.unwrap_or(false) {
            return Err(PanelError::Core(format!(
                "xray binary not found at {}",
                self.binary.display()
            )));
        }

        let mut proc = self.process.write().await;
        if proc.is_some() {
            return Err(PanelError::Core("xray already running".into()));
        }

        let config_path = self.config_path();
        tokio::fs::write(&config_path, serde_json::to_string_pretty(config)?).await?;

        let mut child = Command::new(&self.binary)
            .arg("-c")
            .arg(&config_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stderr = child.stderr.take();
        let last_error = self.last_error.clone();
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let reader = tokio::io::BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        *last_error.write().await = trimmed.to_string();
                    }
                }
            });
        }

        *proc = Some(child);
        *self.start_time.write().await = Some(Instant::now());
        *self.last_error.write().await = String::new();
        tracing::info!("xray started with config {}", config_path.display());
        Ok(())
    }

    async fn stop(&self) -> PanelResult<()> {
        let mut proc = self.process.write().await;
        if let Some(mut child) = proc.take() {
            let _ = child.kill().await;
            *self.start_time.write().await = None;
            tracing::info!("xray stopped");
        }
        Ok(())
    }

    async fn restart(&self, config: &Value) -> PanelResult<()> {
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start(config).await
    }

    async fn is_running(&self) -> bool {
        let proc = self.process.read().await;
        if let Some(ref child) = *proc {
            child.id().is_some()
        } else {
            false
        }
    }

    async fn reload(&self, config: &Value) -> PanelResult<()> {
        // xray does not support hot reload natively; restart instead
        self.restart(config).await
    }

    async fn version(&self) -> PanelResult<String> {
        let output = Command::new(&self.binary).arg("version").output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    async fn uptime_secs(&self) -> PanelResult<u64> {
        Ok(self
            .start_time
            .read()
            .await
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0))
    }

    async fn active_inbounds(&self) -> PanelResult<Vec<String>> {
        // TODO: query xray API for active inbounds
        Ok(vec![])
    }

    async fn last_error(&self) -> PanelResult<String> {
        Ok(self.last_error.read().await.clone())
    }
}

/// sing-box process manager.
pub struct SingBoxProcessManager {
    binary: PathBuf,
    config_dir: PathBuf,
    process: RwLock<Option<Child>>,
    start_time: RwLock<Option<Instant>>,
    last_error: Arc<RwLock<String>>,
}

impl SingBoxProcessManager {
    pub fn new(binary: impl AsRef<Path>, config_dir: impl AsRef<Path>) -> PanelResult<Self> {
        Ok(Self {
            binary: binary.as_ref().to_path_buf(),
            config_dir: config_dir.as_ref().to_path_buf(),
            process: RwLock::new(None),
            start_time: RwLock::new(None),
            last_error: Arc::new(RwLock::new(String::new())),
        })
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("sing-box.json")
    }
}

#[async_trait::async_trait]
impl CoreManager for SingBoxProcessManager {
    fn core_type(&self) -> CoreType {
        CoreType::SingBox
    }

    async fn start(&self, config: &Value) -> PanelResult<()> {
        if !tokio::fs::try_exists(&self.binary).await.unwrap_or(false) {
            return Err(PanelError::Core(format!(
                "sing-box binary not found at {}",
                self.binary.display()
            )));
        }

        let mut proc = self.process.write().await;
        if proc.is_some() {
            return Err(PanelError::Core("sing-box already running".into()));
        }

        let config_path = self.config_path();
        tokio::fs::write(&config_path, serde_json::to_string_pretty(config)?).await?;

        let mut child = Command::new(&self.binary)
            .arg("run")
            .arg("-c")
            .arg(&config_path)
            .arg("-D")
            .arg(&self.config_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stderr = child.stderr.take();
        let last_error = self.last_error.clone();
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        *last_error.write().await = trimmed.to_string();
                    }
                }
            });
        }

        *proc = Some(child);
        *self.start_time.write().await = Some(Instant::now());
        *self.last_error.write().await = String::new();
        tracing::info!("sing-box started with config {}", config_path.display());
        Ok(())
    }

    async fn stop(&self) -> PanelResult<()> {
        let mut proc = self.process.write().await;
        if let Some(mut child) = proc.take() {
            let _ = child.kill().await;
            *self.start_time.write().await = None;
            tracing::info!("sing-box stopped");
        }
        Ok(())
    }

    async fn restart(&self, config: &Value) -> PanelResult<()> {
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start(config).await
    }

    async fn is_running(&self) -> bool {
        let proc = self.process.read().await;
        if let Some(ref child) = *proc {
            child.id().is_some()
        } else {
            false
        }
    }

    async fn reload(&self, config: &Value) -> PanelResult<()> {
        // sing-box supports reload via SIGHUP or `sing-box reload`
        // For now we write config and use restart; can be optimized later
        let config_path = self.config_path();
        tokio::fs::write(&config_path, serde_json::to_string_pretty(config)?).await?;

        let output = Command::new(&self.binary)
            .arg("reload")
            .arg("-c")
            .arg(&config_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PanelError::Core(format!(
                "sing-box reload failed: {}",
                stderr
            )));
        }

        tracing::info!("sing-box config reloaded");
        Ok(())
    }

    async fn version(&self) -> PanelResult<String> {
        let output = Command::new(&self.binary).arg("version").output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    async fn uptime_secs(&self) -> PanelResult<u64> {
        Ok(self
            .start_time
            .read()
            .await
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0))
    }

    async fn active_inbounds(&self) -> PanelResult<Vec<String>> {
        // TODO: query sing-box API for active inbounds
        Ok(vec![])
    }

    async fn last_error(&self) -> PanelResult<String> {
        Ok(self.last_error.read().await.clone())
    }
}

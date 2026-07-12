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
            let msg = format!("xray binary not found at {}", self.binary.display());
            *self.last_error.write().await = msg.clone();
            return Err(PanelError::Core(msg));
        }

        let mut proc = self.process.write().await;
        if proc.is_some() {
            return Err(PanelError::Core("xray already running".into()));
        }

        let config_path = self.config_path();
        if let Err(e) = tokio::fs::write(&config_path, serde_json::to_string_pretty(config)?).await
        {
            let msg = format!("failed to write xray config: {}", e);
            *self.last_error.write().await = msg.clone();
            return Err(PanelError::Core(msg));
        }

        let mut child = match Command::new(&self.binary)
            .arg("-c")
            .arg(&config_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("failed to spawn xray: {}", e);
                *self.last_error.write().await = msg.clone();
                return Err(PanelError::Core(msg));
            }
        };

        let last_error = self.last_error.clone();
        *self.last_error.write().await = String::new();
        if let Some(stderr) = child.stderr.take() {
            spawn_output_reader(stderr, last_error.clone());
        }
        if let Some(stdout) = child.stdout.take() {
            spawn_output_reader(stdout, last_error);
        }

        // Give the process a moment to fail on startup (bad config, missing
        // permissions, etc.) so we can capture the real error message.
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                record_exit(self.last_error.clone(), code).await;
                let err = self.last_error.read().await.clone();
                tracing::error!("xray exited immediately: {}", err);
                return Err(PanelError::Core(err));
            }
            Ok(None) => {
                *proc = Some(child);
                *self.start_time.write().await = Some(Instant::now());
                tracing::info!("xray started with config {}", config_path.display());
                Ok(())
            }
            Err(e) => {
                let msg = format!("failed to check xray status after start: {}", e);
                *self.last_error.write().await = msg.clone();
                Err(PanelError::Core(msg))
            }
        }
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
        let mut proc = self.process.write().await;
        if let Some(ref mut child) = *proc {
            match child.try_wait() {
                Ok(None) => true,
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    record_exit(self.last_error.clone(), code).await;
                    *proc = None;
                    false
                }
                Err(e) => {
                    *self.last_error.write().await =
                        format!("failed to check process status: {}", e);
                    *proc = None;
                    false
                }
            }
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
        Ok(first_output_line(&stdout))
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
            let msg = format!("sing-box binary not found at {}", self.binary.display());
            *self.last_error.write().await = msg.clone();
            return Err(PanelError::Core(msg));
        }

        let mut proc = self.process.write().await;
        if proc.is_some() {
            return Err(PanelError::Core("sing-box already running".into()));
        }

        let config_path = self.config_path();
        if let Err(e) = tokio::fs::write(&config_path, serde_json::to_string_pretty(config)?).await
        {
            let msg = format!("failed to write sing-box config: {}", e);
            *self.last_error.write().await = msg.clone();
            return Err(PanelError::Core(msg));
        }

        let mut child = match Command::new(&self.binary)
            .arg("run")
            .arg("-c")
            .arg(&config_path)
            .arg("-D")
            .arg(&self.config_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("failed to spawn sing-box: {}", e);
                *self.last_error.write().await = msg.clone();
                return Err(PanelError::Core(msg));
            }
        };

        let last_error = self.last_error.clone();
        *self.last_error.write().await = String::new();
        if let Some(stderr) = child.stderr.take() {
            spawn_output_reader(stderr, last_error.clone());
        }
        if let Some(stdout) = child.stdout.take() {
            spawn_output_reader(stdout, last_error);
        }

        // Give the process a moment to fail on startup so we capture the error.
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                record_exit(self.last_error.clone(), code).await;
                let err = self.last_error.read().await.clone();
                tracing::error!("sing-box exited immediately: {}", err);
                return Err(PanelError::Core(err));
            }
            Ok(None) => {
                *proc = Some(child);
                *self.start_time.write().await = Some(Instant::now());
                tracing::info!("sing-box started with config {}", config_path.display());
                Ok(())
            }
            Err(e) => {
                let msg = format!("failed to check sing-box status after start: {}", e);
                *self.last_error.write().await = msg.clone();
                Err(PanelError::Core(msg))
            }
        }
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
        let mut proc = self.process.write().await;
        if let Some(ref mut child) = *proc {
            match child.try_wait() {
                Ok(None) => true,
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    record_exit(self.last_error.clone(), code).await;
                    *proc = None;
                    false
                }
                Err(e) => {
                    *self.last_error.write().await =
                        format!("failed to check process status: {}", e);
                    *proc = None;
                    false
                }
            }
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
        Ok(first_output_line(&stdout))
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

//! mihomo (Clash.Meta) process manager.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use pp_common::{CoreType, LifecycleGuard, PanelError, PanelResult};
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::RwLock;

use super::path::absolutize;
use super::{CoreManager, first_output_line, record_exit, spawn_output_reader};
use crate::lifecycle;

/// mihomo (Clash.Meta) process manager.
///
/// The hub always pushes config as JSON; mihomo only accepts YAML config
/// files, so the JSON is serialized to `mihomo.yaml` on start. mihomo has an
/// API-based hot reload (`PUT /configs?force=true` with per-listener diffing)
/// but it requires the external controller; for now reload falls back to a
/// full restart, same as sing-box.
///
/// 子进程为 std Child（tokio 无 `Child::from_std`，详见 lifecycle::CoreSpawn）。
pub struct MihomoProcessManager {
    binary: PathBuf,
    config_dir: PathBuf,
    process: RwLock<Option<std::process::Child>>,
    // 父子绑定句柄（ADR-0012 D1）：声明顺序在 process 之后——manager drop 时
    // 先释放 Child（detach），再由 guard 关闭 Job 句柄杀死残留子进程。
    guard: RwLock<Option<LifecycleGuard>>,
    start_time: RwLock<Option<Instant>>,
    last_error: Arc<RwLock<String>>,
}

impl MihomoProcessManager {
    pub fn new(binary: impl AsRef<Path>, config_dir: impl AsRef<Path>) -> PanelResult<Self> {
        Ok(Self {
            binary: absolutize(binary.as_ref()),
            config_dir: absolutize(config_dir.as_ref()),
            process: RwLock::new(None),
            guard: RwLock::new(None),
            start_time: RwLock::new(None),
            last_error: Arc::new(RwLock::new(String::new())),
        })
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("mihomo.yaml")
    }

    fn pid_path(&self) -> PathBuf {
        self.config_dir.join("mihomo.pid")
    }
}

#[async_trait::async_trait]
impl CoreManager for MihomoProcessManager {
    fn core_type(&self) -> CoreType {
        CoreType::Mihomo
    }

    async fn start(&self, config: &Value) -> PanelResult<()> {
        if !tokio::fs::try_exists(&self.binary).await.unwrap_or(false) {
            let msg = format!("mihomo binary not found at {}", self.binary.display());
            *self.last_error.write().await = msg.clone();
            return Err(PanelError::Core(msg));
        }

        let mut proc = self.process.write().await;
        if proc.is_some() {
            return Err(PanelError::Core("mihomo already running".into()));
        }

        let yaml = serde_yaml::to_string(config)
            .map_err(|e| PanelError::Core(format!("failed to serialize mihomo config: {}", e)))?;
        let config_path = self.config_path();
        if let Err(e) = tokio::fs::write(&config_path, yaml).await {
            let msg = format!("failed to write mihomo config: {}", e);
            *self.last_error.write().await = msg.clone();
            return Err(PanelError::Core(msg));
        }

        // ADR-0012 D2：收割上次实例遗留的核心进程（OS 级绑定未覆盖的旧版本
        // 孤儿、macOS 等场景），避免端口被残留进程占用。
        lifecycle::reap_stale(&self.pid_path(), &self.binary);

        // ADR-0012 D1：OS 级父子绑定 spawn（父进程死亡核心即被 OS 回收）。
        let spawn = match lifecycle::spawn_core(
            "mihomo",
            &self.binary,
            [
                OsStr::new("-d"),
                self.config_dir.as_os_str(),
                OsStr::new("-f"),
                config_path.as_os_str(),
            ],
        ) {
            Ok(s) => s,
            Err(msg) => {
                *self.last_error.write().await = msg.clone();
                return Err(PanelError::Core(msg));
            }
        };

        let mut child = spawn.child;
        let last_error = self.last_error.clone();
        *self.last_error.write().await = String::new();
        if let Some(stderr) = spawn.stderr {
            spawn_output_reader(stderr, last_error.clone());
        }
        if let Some(stdout) = spawn.stdout {
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
                tracing::error!("mihomo exited immediately: {}", err);
                Err(PanelError::Core(err))
            }
            Ok(None) => {
                lifecycle::write_pid_file(&self.pid_path(), child.id(), &self.binary);
                *self.guard.write().await = Some(spawn.guard);
                *proc = Some(child);
                *self.start_time.write().await = Some(Instant::now());
                tracing::info!("mihomo started with config {}", config_path.display());
                Ok(())
            }
            Err(e) => {
                let msg = format!("failed to check mihomo status after start: {}", e);
                *self.last_error.write().await = msg.clone();
                Err(PanelError::Core(msg))
            }
        }
    }

    async fn stop(&self) -> PanelResult<()> {
        let mut proc = self.process.write().await;
        if let Some(mut child) = proc.take() {
            // std Child：kill 后同步 wait 收尸，避免僵尸进程。
            let _ = child.kill();
            let _ = child.wait();
            *self.start_time.write().await = None;
            tracing::info!("mihomo stopped");
        }
        // ADR-0012：释放父子绑定句柄（Windows Job 句柄关闭会确保子进程死亡）
        // 并清理 PID 文件。
        *self.guard.write().await = None;
        lifecycle::remove_pid_file(&self.pid_path());
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
                    *self.guard.write().await = None;
                    lifecycle::remove_pid_file(&self.pid_path());
                    false
                }
                Err(e) => {
                    *self.last_error.write().await =
                        format!("failed to check process status: {}", e);
                    *proc = None;
                    *self.guard.write().await = None;
                    lifecycle::remove_pid_file(&self.pid_path());
                    false
                }
            }
        } else {
            false
        }
    }

    async fn reload(&self, config: &Value) -> PanelResult<()> {
        // mihomo hot reload requires the external controller API; restart instead
        self.restart(config).await
    }

    async fn version(&self) -> PanelResult<String> {
        let output = pp_common::no_window_tokio(Command::new(&self.binary).arg("-v"))
            .output()
            .await?;
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
        Ok(vec![])
    }

    async fn last_error(&self) -> PanelResult<String> {
        Ok(self.last_error.read().await.clone())
    }

    fn reap_stale(&self) {
        lifecycle::reap_stale(&self.pid_path(), &self.binary);
    }
}

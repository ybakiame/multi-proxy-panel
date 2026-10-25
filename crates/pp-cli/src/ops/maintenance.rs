//! Cross-command maintenance locking and machine-readable upgrade status.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::io::AsyncWriteExt;

#[derive(Debug)]
pub struct MaintenanceLock {
    _file: File,
}

impl MaintenanceLock {
    /// Acquire the shared maintenance lock without waiting for another command.
    pub fn acquire(opt_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(opt_dir)
            .with_context(|| format!("failed to create {}", opt_dir.display()))?;
        let path = opt_dir.join("maintenance.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("failed to open maintenance lock {}", path.display()))?;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
                bail!(
                    "另一个 install/upgrade/rollback 正在运行（锁文件: {}）",
                    path.display()
                );
            }
            return Err(error).with_context(|| format!("failed to lock {}", path.display()));
        }
        Ok(Self { _file: file })
    }

    #[cfg(test)]
    fn file(&self) -> &File {
        &self._file
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeOutcome {
    Running,
    Succeeded,
    Failed,
    RolledBack,
}

#[derive(Debug, Serialize)]
struct StatusRecord {
    status: UpgradeOutcome,
    stage: String,
    component: String,
    current_version: Option<String>,
    target_version: Option<String>,
    message: String,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

pub struct UpgradeStatus {
    path: Option<PathBuf>,
    record: StatusRecord,
}

impl UpgradeStatus {
    pub fn new(path: Option<PathBuf>, component: &str, target_version: Option<String>) -> Self {
        Self {
            path,
            record: StatusRecord {
                status: UpgradeOutcome::Running,
                stage: "resolving".to_string(),
                component: component.to_string(),
                current_version: None,
                target_version,
                message: String::new(),
                started_at: Utc::now(),
                finished_at: None,
            },
        }
    }

    pub async fn stage(&mut self, stage: &str, message: &str) -> Result<()> {
        self.record.status = UpgradeOutcome::Running;
        self.record.stage = stage.to_string();
        self.record.message = message.to_string();
        self.write().await
    }

    pub fn set_current_version(&mut self, version: Option<String>) {
        self.record.current_version = version;
    }

    pub fn set_target_version(&mut self, version: String) {
        self.record.target_version = Some(version);
    }

    pub async fn finish(&mut self, outcome: UpgradeOutcome, message: &str) -> Result<()> {
        self.record.status = outcome;
        self.record.message = message.to_string();
        self.record.finished_at = Some(Utc::now());
        self.write().await
    }

    async fn write(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        let data = serde_json::to_vec_pretty(&self.record)?;
        let mut file = tokio::fs::File::create(&temporary)
            .await
            .with_context(|| format!("failed to create {}", temporary.display()))?;
        file.write_all(&data).await?;
        file.write_all(b"\n").await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, path)
            .await
            .with_context(|| format!("failed to replace status file {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflicting_lock_is_reported() {
        let temp = tempfile::tempdir().unwrap();
        let _first = MaintenanceLock::acquire(temp.path()).unwrap();
        let error = MaintenanceLock::acquire(temp.path()).unwrap_err();
        assert!(error.to_string().contains("另一个"));
    }

    #[test]
    fn lock_is_released_on_drop() {
        let temp = tempfile::tempdir().unwrap();
        let first = MaintenanceLock::acquire(temp.path()).unwrap();
        let result = unsafe { libc::flock(first.file().as_raw_fd(), libc::LOCK_NB) };
        assert_eq!(result, -1);
        drop(first);
        assert!(MaintenanceLock::acquire(temp.path()).is_ok());
    }

    #[tokio::test]
    async fn status_is_written_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("status.json");
        let mut status = UpgradeStatus::new(Some(path.clone()), "hub", Some("latest".into()));
        status.set_current_version(Some("1.0.0".into()));
        status.set_target_version("1.1.0".into());
        status
            .stage("downloading", "download started")
            .await
            .unwrap();
        status
            .finish(UpgradeOutcome::Succeeded, "done")
            .await
            .unwrap();

        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["status"], "succeeded");
        assert_eq!(value["stage"], "downloading");
        assert_eq!(value["current_version"], "1.0.0");
        assert_eq!(value["target_version"], "1.1.0");
        assert!(value["finished_at"].is_string());
    }
}

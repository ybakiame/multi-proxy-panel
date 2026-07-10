use pp_common::{CoreType, PanelResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::installer::ensure_core_binary;
use super::manager::{CoreManager, CoreManagerFactory};

/// Supervises multiple core processes on a single agent.
pub struct CoreSupervisor {
    bin_dir: RwLock<Option<PathBuf>>,
    config_dir: RwLock<Option<PathBuf>>,
    managers: RwLock<HashMap<CoreType, Arc<dyn CoreManager>>>,
}

impl CoreSupervisor {
    pub fn new() -> Self {
        Self {
            bin_dir: RwLock::new(None),
            config_dir: RwLock::new(None),
            managers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a core manager.
    pub async fn register(&self, manager: Arc<dyn CoreManager>) {
        let mut managers = self.managers.write().await;
        managers.insert(manager.core_type(), manager);
    }

    /// Get a manager for a specific core type. Returns None if no manager is
    /// registered and the binary is not available.
    pub async fn get(&self, core_type: CoreType) -> Option<Arc<dyn CoreManager>> {
        let managers = self.managers.read().await;
        managers.get(&core_type).cloned()
    }

    fn binary_path(bin_dir: &Path, name: &str) -> PathBuf {
        bin_dir.join(name)
    }

    /// Initialize managers from available binaries.
    pub async fn discover(&self, bin_dir: &Path, config_dir: &Path) -> PanelResult<Vec<CoreType>> {
        *self.bin_dir.write().await = Some(bin_dir.to_path_buf());
        *self.config_dir.write().await = Some(config_dir.to_path_buf());

        let mut discovered = Vec::new();

        // Check xray
        let xray_path = Self::binary_path(bin_dir, "xray");
        if xray_path.exists() {
            if let Ok(manager) = CoreManagerFactory::create(CoreType::Xray, &xray_path, config_dir)
            {
                self.register(Arc::from(manager)).await;
                discovered.push(CoreType::Xray);
            }
        } else {
            tracing::warn!(
                "xray binary not found at {}, will install on demand",
                xray_path.display()
            );
        }

        // Check sing-box
        let singbox_path = Self::binary_path(bin_dir, "sing-box");
        if singbox_path.exists() {
            if let Ok(manager) =
                CoreManagerFactory::create(CoreType::SingBox, &singbox_path, config_dir)
            {
                self.register(Arc::from(manager)).await;
                discovered.push(CoreType::SingBox);
            }
        } else {
            tracing::warn!(
                "sing-box binary not found at {}, will install on demand",
                singbox_path.display()
            );
        }

        Ok(discovered)
    }

    /// Ensure a manager exists for `core_type`. If the binary is missing, it is
    /// downloaded from the upstream release and a manager is registered.
    pub async fn ensure_manager(
        &self,
        core_type: CoreType,
        bin_dir: &Path,
        config_dir: &Path,
        version: Option<&str>,
    ) -> PanelResult<Arc<dyn CoreManager>> {
        if let Some(manager) = self.get(core_type).await {
            return Ok(manager);
        }

        let binary_path = ensure_core_binary(bin_dir, core_type, version).await?;
        let manager = CoreManagerFactory::create(core_type, &binary_path, config_dir)?;
        let manager: Arc<dyn CoreManager> = Arc::from(manager);
        self.register(manager.clone()).await;
        Ok(manager)
    }

    /// Ensure a manager using the directories provided to the most recent
    /// `discover` call.
    pub async fn ensure_manager_from_discovered(
        &self,
        core_type: CoreType,
        version: Option<&str>,
    ) -> PanelResult<Arc<dyn CoreManager>> {
        let bin_dir = self.bin_dir.read().await.clone().ok_or_else(|| {
            pp_common::PanelError::Core("supervisor has not been discovered with a bin_dir".into())
        })?;
        let config_dir = self.config_dir.read().await.clone().ok_or_else(|| {
            pp_common::PanelError::Core(
                "supervisor has not been discovered with a config_dir".into(),
            )
        })?;
        self.ensure_manager(core_type, &bin_dir, &config_dir, version)
            .await
    }

    /// Stop all managed cores.
    pub async fn stop_all(&self) -> PanelResult<()> {
        let managers = self.managers.read().await;
        for (ty, manager) in managers.iter() {
            if let Err(e) = manager.stop().await {
                tracing::warn!("failed to stop {:?}: {}", ty, e);
            }
        }
        Ok(())
    }
}

impl Default for CoreSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

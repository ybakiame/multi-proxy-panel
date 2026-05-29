use pp_common::{CoreType, PanelResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::manager::{CoreManager, CoreManagerFactory};

/// Supervises multiple core processes on a single agent.
pub struct CoreSupervisor {
    managers: RwLock<HashMap<CoreType, Arc<dyn CoreManager>>>,
}

impl CoreSupervisor {
    pub fn new() -> Self {
        Self {
            managers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a core manager.
    pub async fn register(&self, manager: Arc<dyn CoreManager>) {
        let mut managers = self.managers.write().await;
        managers.insert(manager.core_type(), manager);
    }

    /// Get a manager for a specific core type.
    pub async fn get(&self, core_type: CoreType) -> Option<Arc<dyn CoreManager>> {
        let managers = self.managers.read().await;
        managers.get(&core_type).cloned()
    }

    /// Initialize managers from available binaries.
    pub async fn discover(
        &self,
        config_dir: &std::path::Path,
    ) -> PanelResult<Vec<CoreType>> {
        let mut discovered = Vec::new();

        // Check xray
        if let Ok(manager) = CoreManagerFactory::create(
            CoreType::Xray,
            "xray",
            config_dir,
        ) {
            self.register(Arc::from(manager)).await;
            discovered.push(CoreType::Xray);
        }

        // Check sing-box
        if let Ok(manager) = CoreManagerFactory::create(
            CoreType::SingBox,
            "sing-box",
            config_dir,
        ) {
            self.register(Arc::from(manager)).await;
            discovered.push(CoreType::SingBox);
        }

        Ok(discovered)
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

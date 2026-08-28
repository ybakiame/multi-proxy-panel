//! Scheduler-related methods for [`ClientState`].

use pp_common::PanelResult;
use pp_script::{FilePersistentStore, ScriptHost, ScriptLimits, ScriptScheduler, TaskScript};
use std::sync::Arc;

use crate::http_exec::ReqwestHttpExecutor;
use crate::remote::RemoteManager;
use crate::state::ClientState;

impl ClientState {
    /// Start scheduled task scheduler independently from remote cache (phase ③ decoupling: does not depend on MITM).
    ///
    /// Read task scripts from `remote_cache/`, construct ScriptHost (HttpExecutor + Notifier +
    /// FilePersistentStore) and start scheduler. Returns error on failure, caller decides whether to log warning.
    pub(crate) async fn start_scheduler_from_cache(&mut self) -> PanelResult<()> {
        let remote = RemoteManager::new(self.config.data_dir.clone());
        let merged = remote.load_cached()?;
        let host = Arc::new(ScriptHost::new(
            Arc::new(ReqwestHttpExecutor::new()),
            Arc::new(FilePersistentStore::new(
                self.config.data_dir.join("script_store"),
            )),
            Arc::clone(&self.notifier),
        ));
        self.start_scheduler(host, merged.task_scripts).await
    }

    /// Start remote subscription task script scheduler; tasks with illegal cron expressions are skipped and logged as warnings.
    pub(crate) async fn start_scheduler(
        &mut self,
        host: Arc<ScriptHost>,
        tasks: Vec<TaskScript>,
    ) -> PanelResult<()> {
        let mut scheduler = ScriptScheduler::new(host, ScriptLimits::default());
        let mut registered = 0usize;
        for task in tasks {
            if !task.enabled {
                continue;
            }
            match scheduler.add_task(task) {
                Ok(()) => registered += 1,
                Err(e) => {
                    tracing::warn!(error = %e, "skip scheduled task with invalid cron expression")
                }
            }
        }
        if registered == 0 {
            return Ok(());
        }
        let scheduler = Arc::new(scheduler);
        let handle = Arc::clone(&scheduler).start().await?;
        self.scheduler = Some(super::SchedulerHandle { scheduler, handle });
        Ok(())
    }

    /// Stop scheduler: send stop signal and wait for background loop to exit.
    pub(crate) async fn stop_scheduler(&mut self) {
        use std::time::Duration;
        if let Some(handle) = self.scheduler.take() {
            handle.scheduler.stop().await;
            let _ = tokio::time::timeout(Duration::from_secs(5), handle.handle).await;
        }
    }
}

//! MITM-related methods for [`ClientState`].

use pp_common::PanelResult;
use pp_mitm::{RewriteEngine, ScriptHookEngine};
use pp_script::{FilePersistentStore, ScriptHost, ScriptLimits};
use std::sync::Arc;

use crate::core_config;
use crate::http_exec::ReqwestHttpExecutor;
use crate::mitm::{MitmBuildOptions, build_mitm_proxy};
use crate::remote::RemoteManager;
use crate::state::ClientState;

impl ClientState {
    /// When MITM is enabled, start MITM and return chain info; when not enabled, return `None`.
    ///
    /// Read remote subscription cache (rewrite rules / script hooks / hostnames), build and start
    /// MITM, upstream points to core return mixed entry (`mixed_port + 1`).
    /// Scheduled task scheduler is already started independently in [`ClientState::start`], no longer managed by MITM chain.
    pub(crate) async fn start_mitm_chain(&mut self) -> PanelResult<Option<core_config::MitmChain>> {
        if !self.config.mitm_enabled {
            return Ok(None);
        }
        tracing::info!("Starting MITM proxy");
        // Read remote subscription cache: rewrite rules / script hooks / hostnames.
        let remote = RemoteManager::new(self.config.data_dir.clone());
        let merged = match remote.load_cached() {
            Ok(m) => m,
            Err(e) => return Err(e),
        };
        // Module argument template replacement: user values (remotes.argument_values) → parameter defaults
        // (metas.arguments) → keep as-is; replacement result is passed through ScriptRule as $argument.
        let remotes = remote.load().unwrap_or_default();
        let hook_rules =
            crate::remote::apply_argument_templates(merged.scripts, &merged.metas, &remotes);
        // Merge whitelist (local config + remote subscription), shared by MITM and core routing rules.
        let mut hostnames = self.config.mitm.hostnames.clone();
        for extra in &merged.hostnames {
            if !hostnames.contains(extra) {
                hostnames.push(extra.clone());
            }
        }
        let rewrite = RewriteEngine {
            rules: merged.rewrites,
        };
        let host = Arc::new(ScriptHost::new(
            Arc::new(ReqwestHttpExecutor::new()),
            Arc::new(FilePersistentStore::new(
                self.config.data_dir.join("script_store"),
            )),
            Arc::clone(&self.notifier),
        ));
        let hooks = ScriptHookEngine::new(
            Arc::clone(&host),
            self.config.mitm.script_dialect,
            ScriptLimits::default(),
            hook_rules,
        );
        // MITM upstream points to core return mixed entry (mixed_port + 1).
        let return_port = self.config.mixed_port + 1;
        let options = MitmBuildOptions {
            extra_hostnames: hostnames.clone(),
            upstream_port: Some(return_port),
            rewrite,
            hooks: Some(hooks),
        };
        let proxy = match build_mitm_proxy(&self.config, options, self.recorder.clone()) {
            Ok(p) => p,
            Err(e) => return Err(e),
        };
        let running = match proxy.start().await {
            Ok(r) => r,
            Err(e) => return Err(e),
        };
        let mitm_addr = running.addr;
        self.mitm = Some(running);
        Ok(Some(core_config::MitmChain {
            proxy_addr: mitm_addr,
            return_port,
            hostnames,
        }))
    }

    /// Rollback after MITM has started and subsequent steps fail: close MITM and scheduler.
    pub(crate) async fn rollback_mitm_started(&mut self) {
        self.stop_mitm().await;
        self.stop_scheduler().await;
    }

    pub(crate) async fn stop_mitm(&mut self) {
        if let Some(m) = self.mitm.take() {
            m.shutdown();
        }
    }
}

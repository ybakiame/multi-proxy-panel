//! Service startup/shutdown methods for [`ClientState`].

#[cfg(target_os = "android")]
use pp_common::CoreType;
use pp_common::PanelResult;
use std::net::{Ipv4Addr, SocketAddr};

use crate::runner::CoreRunner;
use crate::state::ClientState;

impl ClientState {
    /// After subscription and config composition, start core → enable system proxy (pointing to core mixed main entry),
    /// rollback on failure. MITM and scheduler are already started before core in [`ClientState::start`].
    ///
    /// Rollback strategy: if core startup fails, close MITM and scheduler; if system proxy enable fails, close core, MITM and scheduler in reverse order,
    /// then propagate the error upward.
    pub(crate) async fn start_services(
        &mut self,
        config_json: &serde_json::Value,
    ) -> PanelResult<()> {
        // Core startup log: Android core is built-in libbox (driven by Kotlin VpnPlugin), no standalone binary;
        // desktop is external core binary, print its path.
        #[cfg(target_os = "android")]
        tracing::info!("Starting core (Android built-in libbox)");
        #[cfg(not(target_os = "android"))]
        tracing::info!(binary = %self.config.core_binary.display(), "Starting core");
        let core = CoreRunner::create(
            self.config.core_type,
            &self.config.core_binary,
            &self.config.data_dir,
        )?;

        // Before Android startup, write the final config sent to core to disk with credentials redacted: uuid/password/server masked as
        // "***" and written to data_dir/logs/, file name distinguished by core type — sing-box keeps
        // last_start_config.json (pretty JSON), mihomo writes last_start_config.yaml
        // (serde_yaml serialization, consistent with mihomo's actual YAML format). This file is included in log export zip,
        // for troubleshooting to confirm the real config reaching the core. The whole process is best-effort: any
        // IO failure only logs a warning, never affects the startup flow.
        #[cfg(target_os = "android")]
        {
            let logs_dir = self.config.data_dir.join("logs");
            let result = (|| -> std::io::Result<std::path::PathBuf> {
                std::fs::create_dir_all(&logs_dir)?;
                let mut redacted = config_json.clone();
                super::compat::redact_config_credentials(&mut redacted);
                let (file_name, content) = if self.config.core_type == CoreType::Mihomo {
                    (
                        "last_start_config.yaml",
                        serde_yaml::to_string(&redacted).map_err(std::io::Error::other)?,
                    )
                } else {
                    (
                        "last_start_config.json",
                        serde_json::to_string_pretty(&redacted).map_err(std::io::Error::other)?,
                    )
                };
                let path = logs_dir.join(file_name);
                std::fs::write(&path, content)?;
                Ok(path)
            })();
            match result {
                Ok(path) => tracing::info!(
                    path = %path.display(),
                    "Wrote redacted final core config to disk (sing-box as JSON / mihomo as YAML, included in log export zip)"
                ),
                Err(e) => tracing::warn!(
                    error = %e,
                    path = %logs_dir.display(),
                    "Redacted core config write failed (does not affect startup)"
                ),
            }
        }

        if let Err(e) = core.start(config_json).await {
            self.rollback_mitm_started().await;
            return Err(e);
        }
        self.core = Some(core);

        if self.config.system_proxy_enabled {
            // System proxy always points to core mixed main entry (MITM hangs after core).
            let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), self.config.mixed_port);
            if let Err(e) = self.sysproxy.enable(addr).await {
                tracing::error!(%addr, "Enabling system proxy failed, rolling back core and MITM");
                self.stop_core().await;
                self.rollback_mitm_started().await;
                return Err(e);
            }
        }

        Ok(())
    }

    pub(crate) async fn stop_core(&mut self) {
        if let Some(core) = self.core.take()
            && let Err(e) = core.stop().await
        {
            // Stop failure is not silent: when Android bridge stop (VpnService orderly shutdown) fails, log
            // warning for problem diagnosis; frontend polling still reflects real running state via is_running.
            tracing::warn!(error = %e, "Stopping core failed");
        }
    }
}

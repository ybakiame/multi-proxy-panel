//! Post-core Clash API setup: readiness wait, rule mode push, selection replay,
//! and the background connection tracker.

use crate::core_config;
use crate::state::{ClientState, ConnectionTrackerHandle};

impl ClientState {
    /// Runs after the core is started and the Clash API is enabled:
    /// waits for API readiness, pushes the persisted rule mode (sing-box has no
    /// composition-level mode field, so this PATCH is the only way), replays
    /// persisted group selections, and starts the connection tracker.
    /// All steps are best-effort: failures are logged, never fatal.
    pub(crate) async fn post_core_clash_setup(&mut self) {
        // 核心已启动且 Clash API 开启时，best-effort 推送持久化规则模式：mihomo
        // 启动配置已含顶层 `mode`，此推送冗余但无害；sing-box 无组合层 mode 字段，
        // 完全依赖本次 PATCH 让持久化模式生效。失败仅记 warning，不影响启动。
        if self.config.clash_api_enabled {
            // The Clash API may lag behind core startup (notably Android's async
            // VPN boot); wait for readiness so the mode push and the selection
            // replay below don't race it.
            if let Err(e) = core_config::wait_clash_api_ready(
                self.config.clash_api_port,
                &self.config.clash_api_secret,
                std::time::Duration::from_secs(15),
            )
            .await
            {
                tracing::warn!(error = %e, "Clash API 未在预期时间内就绪");
            }
        }

        let rule_mode = self.config.normalized_rule_mode().to_string();
        if self.config.clash_api_enabled
            && let Err(e) = core_config::push_clash_mode(
                self.config.clash_api_port,
                &self.config.clash_api_secret,
                &rule_mode,
            )
            .await
        {
            tracing::warn!(
                error = %e,
                rule_mode = %rule_mode,
                "Clash API 推送规则模式失败"
            );
        }

        // Replay persisted group selections after core startup.
        if self.config.clash_api_enabled {
            crate::proxies::replay_group_selections(
                self.config.clash_api_port,
                &self.config.clash_api_secret,
                &self.config.data_dir,
            )
            .await;
        }

        // Start background connection tracker when Clash API is enabled.
        if self.config.clash_api_enabled {
            let tracker = crate::connections::start_connection_tracker(
                self.config.clash_api_port,
                self.config.clash_api_secret.clone(),
            );
            self.connection_tracker = Some(ConnectionTrackerHandle { tracker });
        }
    }
}

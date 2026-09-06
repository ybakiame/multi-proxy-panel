//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{LocalOverrideStore, RuleSetManager};
use tauri::State;

use crate::state::AppState;

use super::views::*;

/// Build the rule-set status view list from `data_dir`.
///
/// Like every local-override read, goes through
/// [`LocalOverrideStore::ensure_builtin_subscriptions`] so the built-in 5
/// rule sets are listed on first entry (see [`super::rules::load_override_view`]
/// for the shared rationale). Extracted for unit testing without a Tauri runtime.
pub(crate) fn load_rule_set_status_views(
    data_dir: &std::path::Path,
) -> Result<Vec<RuleSetStatusView>, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let manager = RuleSetManager::new(data_dir.to_path_buf());
    Ok(ovr
        .rule_set_subscriptions
        .iter()
        .map(|sub| RuleSetStatusView::from_subscription(sub, &manager))
        .collect())
}

/// List all rule sets with subscription and cache status.
#[tauri::command]
pub fn local_override_rulesets(
    state: State<'_, AppState>,
) -> Result<Vec<RuleSetStatusView>, String> {
    load_rule_set_status_views(&state.data_dir)
}

/// Toggle subscription for a rule set.
#[tauri::command]
pub async fn local_override_toggle_ruleset(
    state: State<'_, AppState>,
    community_id: String,
    subscribed: bool,
) -> Result<bool, String> {
    let store = LocalOverrideStore::new(state.data_dir.clone());
    let mut ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let manager = RuleSetManager::new(state.data_dir.clone());
    let changed = manager
        .toggle_subscription(&mut ovr, &community_id, subscribed)
        .await
        .map_err(|e| format!("failed to toggle rule set: {e}"))?;

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after toggle: {e}"))?;

    Ok(changed)
}

/// Manually update all subscribed rule sets now.
#[tauri::command]
pub async fn local_override_update_rulesets_now(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let store = LocalOverrideStore::new(state.data_dir.clone());
    let mut ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let manager = RuleSetManager::new(state.data_dir.clone());
    let updated = manager
        .update_all_subscribed(&mut ovr)
        .await
        .map_err(|e| format!("failed to update rule sets: {e}"))?;

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after update: {e}"))?;

    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_rulesets_returns_builtin_statuses() {
        // 规则集管理首次进入（文件缺失）即返回内置 5 订阅的状态列表，而非空页。
        let dir = tempfile::tempdir().unwrap();
        let views = load_rule_set_status_views(dir.path()).unwrap();
        assert_eq!(views.len(), 5);
        // 首次未下载任何规则集：均为未订阅、未缓存、从未更新。
        assert!(
            views
                .iter()
                .all(|v| !v.subscribed && !v.singbox_cached && v.last_updated == 0)
        );
        // 状态视图字段来自 ensure 后的同一份数据：id/community_id 完整。
        assert!(views.iter().any(|v| v.community_id == "geosite-cn"));
    }

    #[test]
    fn rulesets_statuses_idempotent_across_reads() {
        let dir = tempfile::tempdir().unwrap();
        let views = load_rule_set_status_views(dir.path()).unwrap();
        assert_eq!(views.len(), 5);
        let views2 = load_rule_set_status_views(dir.path()).unwrap();
        assert_eq!(views2.len(), 5);
    }
}

// ---------------------------------------------------------------------------
// Helpers

//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{LocalOverrideStore, RuleSetManager};
use tauri::State;

use crate::state::AppState;


use super::views::*;

/// List all rule sets with subscription and cache status.
#[tauri::command]
pub fn local_override_rulesets(
    state: State<'_, AppState>,
) -> Result<Vec<RuleSetStatusView>, String> {
    let store = LocalOverrideStore::new(state.data_dir.clone());
    let ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let manager = RuleSetManager::new(state.data_dir.clone());
    let views: Vec<RuleSetStatusView> = ovr
        .rule_set_subscriptions
        .iter()
        .map(|sub| RuleSetStatusView::from_subscription(sub, &manager))
        .collect();

    Ok(views)
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

// ---------------------------------------------------------------------------
// Helpers

//! Scenario template commands (apply / revert with auto rule-set subscription).

use pp_client::local_override::{LocalOverrideStore, RuleMatchType, RuleSetManager};
use tauri::State;

use crate::state::AppState;
/// Apply a scenario template.
///
/// Also auto-subscribes (and downloads) the community rule sets referenced by
/// the generated rules, per ADR-0002 section 3.3.3.
#[tauri::command]
pub async fn local_override_apply_template(
    state: State<'_, AppState>,
    template_id: String,
) -> Result<Vec<String>, String> {
    let store = LocalOverrideStore::new(state.data_dir.clone());
    let mut ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let now_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let ids = pp_client::local_override::apply_template(&mut ovr, &template_id, now_sec)
        .map_err(|e| format!("failed to apply template: {e}"))?;

    // Auto-subscribe community rule sets referenced by generated rules.
    let mut referenced: Vec<String> = Vec::new();
    for core in [&ovr.singbox, &ovr.mihomo] {
        for rule in &core.rules {
            if rule.match_type == RuleMatchType::RuleSet && !referenced.contains(&rule.target) {
                referenced.push(rule.target.clone());
            }
        }
    }
    let manager = RuleSetManager::new(state.data_dir.clone());
    for tag in referenced {
        let needs_subscribe = ovr
            .rule_set_subscriptions
            .iter()
            .any(|sub| sub.community_id == tag && !sub.subscribed);
        if needs_subscribe {
            // Failure to download is non-fatal; subscription stays on and the
            // next update retries (same semantics as manual toggle).
            if let Err(e) = manager.toggle_subscription(&mut ovr, &tag, true).await {
                tracing::warn!(rule_set = %tag, error = %e, "auto-subscribe rule set failed");
            }
        }
    }

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after template apply: {e}"))?;

    Ok(ids)
}

/// Revert (undo) a previously applied template.
#[tauri::command]
pub fn local_override_revert_template(
    state: State<'_, AppState>,
    template_id: String,
) -> Result<bool, String> {
    let store = LocalOverrideStore::new(state.data_dir.clone());
    let mut ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let reverted = pp_client::local_override::revert_template(&mut ovr, &template_id);
    if reverted {
        store
            .save(&ovr)
            .map_err(|e| format!("failed to save after template revert: {e}"))?;
    }

    Ok(reverted)
}


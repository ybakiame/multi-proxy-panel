//! Scenario template commands (apply / revert with auto rule-set subscription).

use pp_client::local_override::{
    LocalOverrideStore, RuleSetManager, RuleSetSubscription, template_auto_subscribed_community_ids,
};
use tauri::State;

use crate::state::AppState;
/// Apply a scenario template.
///
/// `apply_template` auto-subscribes the community rule sets the template
/// depends on and always generates the `rule_set` rules (ADR-0002 §3.3.3).
/// After the overrides are persisted we trigger a best-effort download of
/// those dependency rule sets so the generated rules can resolve; a download
/// failure is only logged and retried by the next manual "update now" — it
/// never fails the apply command.
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

    // Persist (subscriptions were already marked subscribed by apply_template).
    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after template apply: {e}"))?;

    // Best-effort, non-blocking download of the template's dependency rule
    // sets: failures are logged and retried on the next "update now".
    // Built-ins come from the static dependency map; custom templates
    // (`"custom:<id>"`) are scanned from their snapshot rules.
    let deps: Vec<RuleSetSubscription> = template_auto_subscribed_community_ids(&ovr, &template_id)
        .into_iter()
        .filter_map(|community_id| {
            ovr.rule_set_subscriptions
                .iter()
                .find(|s| s.community_id == community_id)
                .cloned()
        })
        .collect();
    if !deps.is_empty() {
        let data_dir = state.data_dir.clone();
        tokio::spawn(async move {
            let manager = RuleSetManager::new(data_dir);
            for sub in deps {
                if let Err(e) = manager.download_rule_set(&sub).await {
                    tracing::warn!(
                        community_id = %sub.community_id,
                        error = %e,
                        "template dependency rule set download failed, retry via update-now"
                    );
                }
            }
        });
    }

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

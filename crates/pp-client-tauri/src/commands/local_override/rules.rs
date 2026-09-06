//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{LocalOverrideStore, RuleSetManager};
use tauri::State;

use crate::state::AppState;

use super::convert::{convert_input_to_model, validate_local_override};
use super::views::*;

// ---------------------------------------------------------------------------

/// Get full local override config.
#[tauri::command]
pub fn local_override_get(state: State<'_, AppState>) -> Result<LocalOverrideView, String> {
    let store = LocalOverrideStore::new(state.data_dir.clone());
    let ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    let manager = RuleSetManager::new(state.data_dir.clone());
    Ok(LocalOverrideView::from_model(&ovr, &manager))
}

/// Save full local override config (frontend edits).
///
/// The `custom_rule_sets` segment is full-replacement (same semantics as
/// `rules`). Manual contents are persisted to disk and backing files of
/// removed / switched custom rule sets are best-effort cleaned before the
/// JSON is written, so injected local rule_set paths always resolve.
#[tauri::command]
pub fn local_override_save(
    state: State<'_, AppState>,
    input: SaveLocalOverrideInput,
) -> Result<(), String> {
    let ovr = convert_input_to_model(input)?;
    validate_local_override(&ovr).map_err(|e| format!("validation failed: {e}"))?;

    let manager = RuleSetManager::new(state.data_dir.clone());
    manager
        .sync_custom_rule_set_files(&ovr.custom_rule_sets)
        .map_err(|e| format!("failed to persist custom rule set files: {e}"))?;

    let store = LocalOverrideStore::new(state.data_dir.clone());
    store
        .save(&ovr)
        .map_err(|e| format!("failed to save local override: {e}"))
}

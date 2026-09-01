//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::LocalOverrideStore;
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
    Ok(LocalOverrideView::from_model(&ovr))
}

/// Save full local override config (frontend edits).
#[tauri::command]
pub fn local_override_save(
    state: State<'_, AppState>,
    input: SaveLocalOverrideInput,
) -> Result<(), String> {
    let ovr = convert_input_to_model(input)?;
    validate_local_override(&ovr).map_err(|e| format!("validation failed: {e}"))?;

    let store = LocalOverrideStore::new(state.data_dir.clone());
    store
        .save(&ovr)
        .map_err(|e| format!("failed to save local override: {e}"))
}


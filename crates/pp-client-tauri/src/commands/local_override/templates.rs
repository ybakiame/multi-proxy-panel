//! Scenario template commands (apply / revert user-defined custom templates).
//!
//! 自「废弃内置模板」起只支持 `"custom:<id>"` 地址；应用后不再自动订阅/下载任何
//! 规则集（快照中 `rule_set` 规则引用由用户自控的 custom rule sets 承载）。

use pp_client::local_override::LocalOverrideStore;
use tauri::State;

use crate::state::AppState;

/// Apply a scenario template.
///
/// 仅接受 `"custom:<id>"`（前缀见 `pp_client` 的 `CUSTOM_TEMPLATE_PREFIX`），把模板
/// 快照规则以新 UUID 复制并头插到 `singbox.rules`，记录 applied_templates 后落盘。
#[tauri::command]
pub async fn local_override_apply_template(
    state: State<'_, AppState>,
    template_id: String,
) -> Result<Vec<String>, String> {
    let store = LocalOverrideStore::new(state.data_dir.clone());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let now_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let ids = pp_client::local_override::apply_template(&mut ovr, &template_id, now_sec)
        .map_err(|e| format!("failed to apply template: {e}"))?;

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
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let reverted = pp_client::local_override::revert_template(&mut ovr, &template_id);
    if reverted {
        store
            .save(&ovr)
            .map_err(|e| format!("failed to save after template revert: {e}"))?;
    }

    Ok(reverted)
}

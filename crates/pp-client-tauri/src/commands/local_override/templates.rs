//! Scenario template commands (apply / revert user-defined custom templates).
//!
//! 自「废弃内置模板」起只支持 `"custom:<id>"` 地址；自「场景模板改为规则引用 +
//! 应用激活」起，apply 只加 applied_templates 记录（**不复制规则**），revert 只
//! 移除记录（**不删规则**），重复 apply 幂等。

use pp_client::local_override::LocalOverrideStore;
use tauri::State;

use crate::state::AppState;

/// Apply a scenario template.
///
/// 仅接受 `"custom:<id>"`（前缀见 `pp_client` 的 `CUSTOM_TEMPLATE_PREFIX`）。
/// 语义：模板存在性校验通过后在 `applied_templates` 记录一条（不复制/删除任何
/// 规则）；返回模板当前的规则 ID 引用列表。重复应用幂等（不重复记录）。
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

    let refs = pp_client::local_override::apply_template(&mut ovr, &template_id, now_sec)
        .map_err(|e| format!("failed to apply template: {e}"))?;

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after template apply: {e}"))?;

    Ok(refs)
}

/// Revert (undo) a previously applied template.
///
/// 只移除 `applied_templates` 中的记录（规则列表不动）；返回是否移除了记录。
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

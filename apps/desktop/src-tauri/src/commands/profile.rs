//! Profile (override template) commands.

use pp_client::{Profile, ProfileStoreV2};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{core_type_str, parse_profile_id};
use crate::state::AppState;

/// Profile list view (aligned with frontend `ProfileView` TS type).
#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub core_type: String,
    pub yaml_bytes: u64,
    pub js_bytes: u64,
    pub yaml_url: Option<String>,
    pub js_url: Option<String>,
}

impl ProfileView {
    pub(crate) fn from_profile(p: &Profile) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            core_type: core_type_str(p.core_type),
            yaml_bytes: p.yaml_override.len() as u64,
            js_bytes: p.js_override.len() as u64,
            yaml_url: p.yaml_url.clone(),
            js_url: p.js_url.clone(),
        }
    }
}

/// Profile detail view (with full override content).
#[derive(Debug, Clone, Serialize)]
pub struct ProfileDetailView {
    pub id: String,
    pub name: String,
    pub core_type: String,
    pub yaml_override: String,
    pub js_override: String,
    pub yaml_url: Option<String>,
    pub js_url: Option<String>,
}

impl ProfileDetailView {
    pub(crate) fn from_profile(p: &Profile) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            core_type: core_type_str(p.core_type),
            yaml_override: p.yaml_override.clone(),
            js_override: p.js_override.clone(),
            yaml_url: p.yaml_url.clone(),
            js_url: p.js_url.clone(),
        }
    }
}

/// List all profiles (`data_dir/profiles.json`).
#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileView>, String> {
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    Ok(profiles.iter().map(ProfileView::from_profile).collect())
}

/// Input for creating a profile.
#[derive(Debug, Deserialize)]
pub struct CreateProfileInput {
    pub name: String,
    pub core_type: String,
}

/// Create a new profile; errors on duplicate name.
#[tauri::command]
pub fn create_profile(
    state: State<'_, AppState>,
    input: CreateProfileInput,
) -> Result<ProfileView, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("模板名称不能为空".to_string());
    }
    let core_type = crate::commands::core_type_from_str(&input.core_type)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profile = store
        .add(&name, core_type)
        .map_err(|e| format!("创建模板失败: {e}"))?;
    Ok(ProfileView::from_profile(&profile))
}

/// Get a single profile detail; errors if not found.
#[tauri::command]
pub fn get_profile(state: State<'_, AppState>, id: String) -> Result<ProfileDetailView, String> {
    let id = parse_profile_id(&id)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    let profile = profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "模板不存在".to_string())?;
    Ok(ProfileDetailView::from_profile(profile))
}

/// Input for updating a profile.
#[derive(Debug, Deserialize)]
pub struct UpdateProfileInput {
    pub id: String,
    pub name: String,
    pub yaml_override: String,
    pub js_override: String,
    #[serde(default)]
    pub yaml_url: Option<String>,
    #[serde(default)]
    pub js_url: Option<String>,
}

/// Update profile editable fields.
#[tauri::command]
pub fn update_profile(state: State<'_, AppState>, input: UpdateProfileInput) -> Result<(), String> {
    let id = parse_profile_id(&input.id)?;
    pp_client::validate_yaml_override(&input.yaml_override)
        .map_err(|e| format!("YAML 复写校验失败: {e}"))?;
    pp_client::validate_js_override(&input.js_override)
        .map_err(|e| format!("JS 复写校验失败: {e}"))?;
    pp_client::validate_remote_url(&input.yaml_url)
        .map_err(|e| format!("远程 YAML URL 校验失败: {e}"))?;
    pp_client::validate_remote_url(&input.js_url)
        .map_err(|e| format!("远程 JS URL 校验失败: {e}"))?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let mut profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    let target = profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| "模板不存在".to_string())?;
    target.name = input.name;
    target.yaml_override = input.yaml_override;
    target.js_override = input.js_override;
    target.yaml_url = pp_client::normalize_optional_url(input.yaml_url);
    target.js_url = pp_client::normalize_optional_url(input.js_url);
    store
        .save(&profiles)
        .map_err(|e| format!("保存复写模板失败: {e}"))
}

/// Delete a profile; errors if not found.
#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = parse_profile_id(&id)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    store
        .remove(id)
        .map_err(|e| format!("删除复写模板失败: {e}"))
}

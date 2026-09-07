//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{
    AppliedTemplate, CoreLocalOverride, CustomRuleSetSource, LocalOverride, LocalRule,
    RuleSetManager,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// View types (shell layer, for frontend contract)
// ---------------------------------------------------------------------------

/// Full local override view (returned by `local_override_get`).
///
/// 自「废弃内置规则集订阅」起不再输出 `rule_set_subscriptions` 段；自定义规则集
/// 状态（含 `cached` / `last_updated`）由 `custom_rule_sets` 段承载，前端统一走本
/// 命令消费。
#[derive(Debug, Clone, Serialize)]
pub struct LocalOverrideView {
    pub singbox: CoreLocalOverrideView,
    pub applied_templates: Vec<AppliedTemplateView>,
    pub custom_rule_sets: Vec<CustomRuleSetView>,
    pub custom_templates: Vec<CustomTemplateView>,
}

/// Per-core local override view.
#[derive(Debug, Clone, Serialize)]
pub struct CoreLocalOverrideView {
    pub rules: Vec<LocalRuleView>,
    pub rule_sets: Vec<LocalRuleSetRefView>,
    pub enabled: bool,
}

/// Local rule card view.
#[derive(Debug, Clone, Serialize)]
pub struct LocalRuleView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub match_type: String,
    pub target: String,
    pub action: String,
    pub no_resolve: bool,
    pub invert: bool,
    pub note: String,
    pub created_at: u64,
    pub sort_order: i32,
}

/// Rule set reference view.
#[derive(Debug, Clone, Serialize)]
pub struct LocalRuleSetRefView {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub kind: String,
    pub source: String,
    pub enabled: bool,
    pub auto_update_interval_minutes: u32,
    pub last_updated: u64,
}

/// Applied template view.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedTemplateView {
    pub template_id: String,
    pub applied_at: u64,
    pub generated_rule_ids: Vec<String>,
}

/// User-defined custom rule set view.
///
/// `source` keeps the model shape (remote url+format / manual content) so the
/// editor can echo the content back; `cached` reflects whether the backing
/// file (manual 落盘 or remote cache) currently exists on disk.
#[derive(Debug, Clone, Serialize)]
pub struct CustomRuleSetView {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub source: CustomRuleSetSource,
    pub enabled: bool,
    pub last_updated: u64,
    pub cached: bool,
}

/// User-defined scenario template view.
///
/// `rules` are the template's snapshot of the selected rule cards (displayed
/// in the create form's checklist and echoed back on the card).
#[derive(Debug, Clone, Serialize)]
pub struct CustomTemplateView {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub rules: Vec<LocalRuleView>,
    pub created_at: u64,
}

// ---------------------------------------------------------------------------
// Conversions (pp-client types → View types)
// ---------------------------------------------------------------------------

impl LocalOverrideView {
    pub(crate) fn from_model(model: &LocalOverride, manager: &RuleSetManager) -> Self {
        Self {
            singbox: CoreLocalOverrideView::from_model(&model.singbox),
            applied_templates: model
                .applied_templates
                .iter()
                .map(AppliedTemplateView::from_model)
                .collect(),
            custom_rule_sets: model
                .custom_rule_sets
                .iter()
                .map(|rs| CustomRuleSetView::from_model(rs, manager))
                .collect(),
            custom_templates: model
                .custom_templates
                .iter()
                .map(CustomTemplateView::from_model)
                .collect(),
        }
    }
}

impl CoreLocalOverrideView {
    pub(crate) fn from_model(model: &CoreLocalOverride) -> Self {
        Self {
            rules: model.rules.iter().map(LocalRuleView::from_model).collect(),
            rule_sets: model
                .rule_sets
                .iter()
                .map(LocalRuleSetRefView::from_model)
                .collect(),
            enabled: model.enabled,
        }
    }
}

impl LocalRuleView {
    pub(crate) fn from_model(model: &LocalRule) -> Self {
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            enabled: model.enabled,
            match_type: format!("{:?}", model.match_type).to_lowercase(),
            target: model.target.clone(),
            action: format!("{:?}", model.action).to_lowercase(),
            no_resolve: model.advanced.no_resolve,
            invert: model.advanced.invert,
            note: model.note.clone(),
            created_at: model.created_at,
            sort_order: model.sort_order,
        }
    }
}

impl LocalRuleSetRefView {
    pub(crate) fn from_model(model: &pp_client::local_override::LocalRuleSetRef) -> Self {
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            tag: model.tag.clone(),
            kind: format!("{:?}", model.kind).to_lowercase(),
            source: match &model.source {
                pp_client::local_override::RuleSetSource::Remote { url } => url.clone(),
                pp_client::local_override::RuleSetSource::Local { path } => path.clone(),
                pp_client::local_override::RuleSetSource::Bundled { name } => name.clone(),
            },
            enabled: model.enabled,
            auto_update_interval_minutes: model.auto_update_interval_minutes,
            last_updated: model.last_updated,
        }
    }
}

impl AppliedTemplateView {
    pub(crate) fn from_model(model: &AppliedTemplate) -> Self {
        Self {
            template_id: model.template_id.clone(),
            applied_at: model.applied_at,
            generated_rule_ids: model.generated_rule_ids.clone(),
        }
    }
}

impl CustomRuleSetView {
    pub(crate) fn from_model(
        model: &pp_client::local_override::CustomRuleSet,
        manager: &RuleSetManager,
    ) -> Self {
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            tag: model.tag.clone(),
            source: model.source.clone(),
            enabled: model.enabled,
            last_updated: model.last_updated,
            cached: manager.has_custom_rule_set_file(model),
        }
    }
}

impl CustomTemplateView {
    pub(crate) fn from_model(model: &pp_client::local_override::CustomTemplate) -> Self {
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            desc: model.desc.clone(),
            rules: model.rules.iter().map(LocalRuleView::from_model).collect(),
            created_at: model.created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input for saving local override (full replacement).
#[derive(Debug, Deserialize)]
pub struct SaveLocalOverrideInput {
    pub singbox: CoreLocalOverrideInput,
    pub applied_templates: Vec<AppliedTemplateInput>,
    /// Full replacement of the custom rule set segment (semantics identical
    /// to `rules`). Uses the shared model type so the manual content / remote
    /// url+format round-trips unchanged.
    #[serde(default)]
    pub custom_rule_sets: Vec<pp_client::local_override::CustomRuleSet>,
    /// Full replacement of the custom template segment (semantics identical to
    /// `rules`). Rules carry the flattened `LocalRuleInput` field shape.
    #[serde(default)]
    pub custom_templates: Vec<CustomTemplateInput>,
}

/// Input for one custom scenario template (full rule snapshot).
#[derive(Debug, Deserialize)]
pub struct CustomTemplateInput {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub rules: Vec<LocalRuleInput>,
    pub created_at: u64,
}

#[derive(Debug, Deserialize)]
pub struct CoreLocalOverrideInput {
    pub rules: Vec<LocalRuleInput>,
    pub rule_sets: Vec<LocalRuleSetRefInput>,
    #[serde(default = "crate::commands::local_override::default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct LocalRuleInput {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "crate::commands::local_override::default_true")]
    pub enabled: bool,
    pub match_type: String,
    pub target: String,
    pub action: String,
    #[serde(default)]
    pub no_resolve: bool,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub note: String,
    pub created_at: u64,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize)]
pub struct LocalRuleSetRefInput {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub kind: String,
    pub source: String,
    #[serde(default = "crate::commands::local_override::default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_update_interval_minutes: u32,
    #[serde(default)]
    pub last_updated: u64,
}

#[derive(Debug, Deserialize)]
pub struct AppliedTemplateInput {
    pub template_id: String,
    pub applied_at: u64,
    pub generated_rule_ids: Vec<String>,
}

#[inline]
#[must_use]
pub(crate) const fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Validation

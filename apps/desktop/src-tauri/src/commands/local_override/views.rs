//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{
    AppliedTemplate, CoreLocalOverride, LocalOverride, LocalRule, RuleSetSubscription,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// View types (shell layer, for frontend contract)
// ---------------------------------------------------------------------------

/// Full local override view (returned by `local_override_get`).
#[derive(Debug, Clone, Serialize)]
pub struct LocalOverrideView {
    pub singbox: CoreLocalOverrideView,
    pub mihomo: CoreLocalOverrideView,
    pub rule_set_subscriptions: Vec<RuleSetSubscriptionView>,
    pub applied_templates: Vec<AppliedTemplateView>,
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

/// Rule set subscription view.
#[derive(Debug, Clone, Serialize)]
pub struct RuleSetSubscriptionView {
    pub id: String,
    pub community_id: String,
    pub display_name: String,
    pub category: String,
    pub subscribed: bool,
    pub singbox_url_template: String,
    pub mihomo_url_template: String,
    pub default_interval_minutes: u32,
}

/// Applied template view.
#[derive(Debug, Clone, Serialize)]
pub struct AppliedTemplateView {
    pub template_id: String,
    pub applied_at: u64,
    pub generated_rule_ids: Vec<String>,
}

/// Rule set status view (with cache info).
#[derive(Debug, Clone, Serialize)]
pub struct RuleSetStatusView {
    pub id: String,
    pub community_id: String,
    pub display_name: String,
    pub category: String,
    pub subscribed: bool,
    pub singbox_cached: bool,
    pub mihomo_cached: bool,
    pub last_updated: u64,
}

// ---------------------------------------------------------------------------
// Conversions (pp-client types → View types)
// ---------------------------------------------------------------------------

impl LocalOverrideView {
    pub(crate) fn from_model(model: &LocalOverride) -> Self {
        Self {
            singbox: CoreLocalOverrideView::from_model(&model.singbox),
            mihomo: CoreLocalOverrideView::from_model(&model.mihomo),
            rule_set_subscriptions: model
                .rule_set_subscriptions
                .iter()
                .map(RuleSetSubscriptionView::from_model)
                .collect(),
            applied_templates: model
                .applied_templates
                .iter()
                .map(AppliedTemplateView::from_model)
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

impl RuleSetSubscriptionView {
    pub(crate) fn from_model(model: &RuleSetSubscription) -> Self {
        Self {
            id: model.id.clone(),
            community_id: model.community_id.clone(),
            display_name: model.display_name.clone(),
            category: format!("{:?}", model.category).to_lowercase(),
            subscribed: model.subscribed,
            singbox_url_template: model.singbox_url_template.clone(),
            mihomo_url_template: model.mihomo_url_template.clone(),
            default_interval_minutes: model.default_interval_minutes,
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

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input for saving local override (full replacement).
#[derive(Debug, Deserialize)]
pub struct SaveLocalOverrideInput {
    pub singbox: CoreLocalOverrideInput,
    pub mihomo: CoreLocalOverrideInput,
    pub rule_set_subscriptions: Vec<RuleSetSubscriptionInput>,
    pub applied_templates: Vec<AppliedTemplateInput>,
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
pub struct RuleSetSubscriptionInput {
    pub id: String,
    pub community_id: String,
    pub display_name: String,
    pub category: String,
    #[serde(default)]
    pub subscribed: bool,
    pub singbox_url_template: String,
    pub mihomo_url_template: String,
    pub default_interval_minutes: u32,
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

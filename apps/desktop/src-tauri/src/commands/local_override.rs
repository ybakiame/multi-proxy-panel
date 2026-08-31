//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{
    AppliedTemplate, CoreLocalOverride, LocalOverride, LocalOverrideStore, LocalRule,
    RuleSetManager, RuleSetSubscription,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

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
    fn from_model(model: &LocalOverride) -> Self {
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
    fn from_model(model: &CoreLocalOverride) -> Self {
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
    fn from_model(model: &LocalRule) -> Self {
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
    fn from_model(model: &pp_client::local_override::LocalRuleSetRef) -> Self {
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
    fn from_model(model: &RuleSetSubscription) -> Self {
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
    fn from_model(model: &AppliedTemplate) -> Self {
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
// ---------------------------------------------------------------------------

/// Validate local override before saving.
///
/// Checks:
/// - Rule IDs are unique within each core.
/// - Targets are non-empty for non-Final rules.
/// - RuleSet references point to subscribed rule sets.
fn validate_local_override(ovr: &LocalOverride) -> Result<(), String> {
    for (core_name, core_ovr) in [("singbox", &ovr.singbox), ("mihomo", &ovr.mihomo)] {
        // Check rule ID uniqueness.
        let mut seen = std::collections::HashSet::new();
        for rule in &core_ovr.rules {
            if !seen.insert(rule.id.clone()) {
                return Err(format!(
                    "duplicate rule id '{}' in {core_name}",
                    rule.id
                ));
            }
        }

        // Check non-empty targets.
        for rule in &core_ovr.rules {
            if !matches!(rule.match_type, pp_client::local_override::RuleMatchType::Final)
                && rule.target.trim().is_empty()
            {
                return Err(format!(
                    "rule '{}' in {core_name} has empty target",
                    rule.id
                ));
            }
        }

        // Check RuleSet references are subscribed.
        for rule in &core_ovr.rules {
            if matches!(rule.match_type, pp_client::local_override::RuleMatchType::RuleSet) {
                let subscribed = ovr
                    .rule_set_subscriptions
                    .iter()
                    .any(|s| s.community_id == rule.target && s.subscribed);
                if !subscribed {
                    return Err(format!(
                        "rule '{}' references unsubscribed rule set '{}'",
                        rule.id, rule.target
                    ));
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Command handlers
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

/// Apply a scenario template.
#[tauri::command]
pub fn local_override_apply_template(
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
// ---------------------------------------------------------------------------

fn convert_input_to_model(input: SaveLocalOverrideInput) -> Result<LocalOverride, String> {
    Ok(LocalOverride {
        singbox: convert_core_input(input.singbox)?,
        mihomo: convert_core_input(input.mihomo)?,
        rule_set_subscriptions: input
            .rule_set_subscriptions
            .into_iter()
            .map(convert_subscription_input)
            .collect(),
        applied_templates: input
            .applied_templates
            .into_iter()
            .map(convert_applied_template_input)
            .collect(),
    })
}

fn convert_core_input(input: CoreLocalOverrideInput) -> Result<CoreLocalOverride, String> {
    Ok(CoreLocalOverride {
        rules: input
            .rules
            .into_iter()
            .map(convert_rule_input)
            .collect::<Result<Vec<_>, _>>()?,
        rule_sets: input
            .rule_sets
            .into_iter()
            .map(convert_rule_set_ref_input)
            .collect::<Result<Vec<_>, _>>()?,
        enabled: input.enabled,
    })
}

fn convert_rule_input(input: LocalRuleInput) -> Result<LocalRule, String> {
    let match_type = parse_match_type(&input.match_type)?;
    let action = parse_action(&input.action)?;
    Ok(LocalRule {
        id: input.id,
        name: input.name,
        enabled: input.enabled,
        match_type,
        target: input.target,
        action,
        advanced: pp_client::local_override::RuleAdvancedOptions {
            no_resolve: input.no_resolve,
            invert: input.invert,
            _sniff: false,
        },
        note: input.note,
        created_at: input.created_at,
        sort_order: input.sort_order,
    })
}

fn parse_match_type(s: &str) -> Result<pp_client::local_override::RuleMatchType, String> {
    match s {
        "domain" => Ok(pp_client::local_override::RuleMatchType::Domain),
        "domain_suffix" => Ok(pp_client::local_override::RuleMatchType::DomainSuffix),
        "domain_keyword" => Ok(pp_client::local_override::RuleMatchType::DomainKeyword),
        "ip_cidr" => Ok(pp_client::local_override::RuleMatchType::IpCidr),
        "source_ip_cidr" => Ok(pp_client::local_override::RuleMatchType::SourceIpCidr),
        "rule_set" => Ok(pp_client::local_override::RuleMatchType::RuleSet),
        #[cfg(target_os = "android")]
        "app_package" => Ok(pp_client::local_override::RuleMatchType::AppPackage),
        #[cfg(not(target_os = "android"))]
        "process_name" => Ok(pp_client::local_override::RuleMatchType::ProcessName),
        "port" => Ok(pp_client::local_override::RuleMatchType::Port),
        "final" => Ok(pp_client::local_override::RuleMatchType::Final),
        _ => Err(format!("unknown match_type: {s}")),
    }
}

fn parse_action(s: &str) -> Result<pp_client::local_override::RuleAction, String> {
    match s {
        "proxy" => Ok(pp_client::local_override::RuleAction::Proxy),
        "direct" => Ok(pp_client::local_override::RuleAction::Direct),
        "reject" => Ok(pp_client::local_override::RuleAction::Reject),
        _ => {
            if let Some(tag) = s.strip_prefix("outbound:") {
                Ok(pp_client::local_override::RuleAction::Outbound {
                    tag: tag.to_string(),
                })
            } else {
                Err(format!("unknown action: {s}"))
            }
        }
    }
}

fn convert_rule_set_ref_input(
    input: LocalRuleSetRefInput,
) -> Result<pp_client::local_override::LocalRuleSetRef, String> {
    let kind = parse_rule_set_kind(&input.kind)?;
    let source = pp_client::local_override::RuleSetSource::Remote {
        url: input.source,
    };
    Ok(pp_client::local_override::LocalRuleSetRef {
        id: input.id,
        name: input.name,
        tag: input.tag,
        kind,
        source,
        enabled: input.enabled,
        auto_update_interval_minutes: input.auto_update_interval_minutes,
        last_updated: input.last_updated,
    })
}

fn parse_rule_set_kind(s: &str) -> Result<pp_client::local_override::RuleSetKind, String> {
    match s {
        "singbox_remote" => Ok(pp_client::local_override::RuleSetKind::SingBoxRemote),
        "singbox_local" => Ok(pp_client::local_override::RuleSetKind::SingBoxLocal),
        "mihomo_http" => Ok(pp_client::local_override::RuleSetKind::MihomoHttp),
        "mihomo_file" => Ok(pp_client::local_override::RuleSetKind::MihomoFile),
        _ => Err(format!("unknown rule_set kind: {s}")),
    }
}

fn convert_subscription_input(input: RuleSetSubscriptionInput) -> RuleSetSubscription {
    RuleSetSubscription {
        id: input.id,
        community_id: input.community_id,
        display_name: input.display_name,
        category: parse_rule_set_category(&input.category),
        subscribed: input.subscribed,
        singbox_url_template: input.singbox_url_template,
        mihomo_url_template: input.mihomo_url_template,
        default_interval_minutes: input.default_interval_minutes,
    }
}

fn parse_rule_set_category(s: &str) -> pp_client::local_override::RuleSetCategory {
    match s {
        "geoip" => pp_client::local_override::RuleSetCategory::Geoip,
        "geosite" => pp_client::local_override::RuleSetCategory::Geosite,
        "ads" => pp_client::local_override::RuleSetCategory::Ads,
        "privacy" => pp_client::local_override::RuleSetCategory::Privacy,
        "malware" => pp_client::local_override::RuleSetCategory::Malware,
        _ => pp_client::local_override::RuleSetCategory::Custom,
    }
}

fn convert_applied_template_input(input: AppliedTemplateInput) -> AppliedTemplate {
    AppliedTemplate {
        template_id: input.template_id,
        applied_at: input.applied_at,
        generated_rule_ids: input.generated_rule_ids,
    }
}

impl RuleSetStatusView {
    fn from_subscription(
        sub: &pp_client::local_override::RuleSetSubscription,
        manager: &RuleSetManager,
    ) -> Self {
        Self {
            id: sub.id.clone(),
            community_id: sub.community_id.clone(),
            display_name: sub.display_name.clone(),
            category: format!("{:?}", sub.category).to_lowercase(),
            subscribed: sub.subscribed,
            singbox_cached: manager.is_cached(&sub.community_id, pp_common::CoreType::SingBox),
            mihomo_cached: manager.is_cached(&sub.community_id, pp_common::CoreType::Mihomo),
            last_updated: 0,
        }
    }
}

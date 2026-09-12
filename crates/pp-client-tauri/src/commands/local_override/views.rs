//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management and rule set
//! control.

use pp_client::local_override::{
    CoreLocalOverride, CustomRuleSetSource, LocalOverride, LocalRule, RuleSetManager,
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
///
/// 自「移除场景模板」起 `applied_templates` / `custom_templates` 恒为空数组
/// （serde 兼容字段，前端清理在下一阶段）。
#[derive(Debug, Clone, Serialize)]
pub struct LocalOverrideView {
    pub singbox: CoreLocalOverrideView,
    pub applied_templates: Vec<serde_json::Value>,
    pub custom_rule_sets: Vec<CustomRuleSetView>,
    pub custom_templates: Vec<serde_json::Value>,
}

/// Per-core local override view.
///
/// `enabled` is a **compatibility field**: the model's master switch was removed
/// (rules / rule sets carry their own `enabled`), but the frontend
/// `MasterSwitchCard` still consumes it until the frontend cleanup lands. It is
/// therefore always serialized as `true` and ignored on save.
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

/// User-defined custom rule set view.
///
/// `source` keeps the model shape (remote url+format / manual content) so the
/// editor can echo the content back; `cached` reflects whether the backing
/// file (manual 落盘 or remote cache) currently exists on disk.
///
/// 自「规则集移除 enabled」起不再输出 `enabled`（纯资源，旧文件中的该字段由
/// serde 忽略，不再出现在视图契约里）。
#[derive(Debug, Clone, Serialize)]
pub struct CustomRuleSetView {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub source: CustomRuleSetSource,
    /// Local cache write time (Unix seconds; 0 = never downloaded).
    pub last_updated: u64,
    /// Remote `Last-Modified` (Unix seconds; 0 = unknown). `> last_updated`
    /// means the remote has a newer version (frontend "有更新" chip).
    pub remote_updated_at: u64,
    pub cached: bool,
}

/// Aggregated rule set update outcome (single or batch smart update).
///
/// `skipped` = remote `Last-Modified` ≤ local `last_updated` (already latest);
/// `failed` = HEAD / download error (best-effort per entry).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RuleSetUpdateOutcomeView {
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
}

impl From<pp_client::local_override::RuleSetUpdateOutcome> for RuleSetUpdateOutcomeView {
    fn from(o: pp_client::local_override::RuleSetUpdateOutcome) -> Self {
        Self {
            updated: o.updated,
            skipped: o.skipped,
            failed: o.failed,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions (pp-client types → View types)
// ---------------------------------------------------------------------------

impl LocalOverrideView {
    pub(crate) fn from_model(model: &LocalOverride, manager: &RuleSetManager) -> Self {
        Self {
            singbox: CoreLocalOverrideView::from_model(&model.singbox),
            // 场景模板已移除：恒输出空数组（serde 兼容字段）。
            applied_templates: Vec::new(),
            custom_rule_sets: model
                .custom_rule_sets
                .iter()
                .map(|rs| CustomRuleSetView::from_model(rs, manager))
                .collect(),
            custom_templates: Vec::new(),
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
            // 兼容字段：模型总开关已移除，恒输出 true（前端 MasterSwitchCard 待 T3 清理）。
            enabled: true,
        }
    }
}

impl LocalRuleView {
    pub(crate) fn from_model(model: &LocalRule) -> Self {
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            enabled: model.enabled,
            match_type: rule_match_type_str(&model.match_type),
            target: model.target.clone(),
            action: rule_action_str(&model.action),
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
            kind: rule_set_kind_str(&model.kind),
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
            last_updated: model.last_updated,
            remote_updated_at: model.remote_updated_at,
            cached: manager.has_custom_rule_set_file(model),
        }
    }
}

// ---------------------------------------------------------------------------
// Enum → string mapping (frontend contract)
// ---------------------------------------------------------------------------

/// Canonical `match_type` string (snake_case, mirrors the serde values and is
/// symmetric with `convert.rs::parse_match_type`).
fn rule_match_type_str(t: &pp_client::local_override::RuleMatchType) -> String {
    use pp_client::local_override::RuleMatchType as M;
    match t {
        M::Domain => "domain",
        M::DomainSuffix => "domain_suffix",
        M::DomainKeyword => "domain_keyword",
        M::IpCidr => "ip_cidr",
        M::SourceIpCidr => "source_ip_cidr",
        M::RuleSet => "rule_set",
        #[cfg(target_os = "android")]
        M::AppPackage => "app_package",
        #[cfg(not(target_os = "android"))]
        M::ProcessName => "process_name",
        M::Port => "port",
        M::Final => "final",
    }
    .to_string()
}

/// Canonical `action` string, symmetric with `convert.rs::parse_action`.
///
/// Unit variants map to their word values; the data-carrying `Outbound`
/// variant serializes as `"outbound:<tag>"` (this is the wire string the
/// frontend echoes back and `parse_action` understands — not the enum's
/// serde JSON shape, which is only used for `local_override.json` on disk).
fn rule_action_str(a: &pp_client::local_override::RuleAction) -> String {
    use pp_client::local_override::RuleAction as A;
    match a {
        A::Proxy => "proxy".to_string(),
        A::Direct => "direct".to_string(),
        A::Reject => "reject".to_string(),
        A::Outbound { tag } => format!("outbound:{tag}"),
    }
}

/// Canonical rule-set `kind` string, symmetric with
/// `convert.rs::parse_rule_set_kind`.
fn rule_set_kind_str(k: &pp_client::local_override::RuleSetKind) -> String {
    use pp_client::local_override::RuleSetKind as K;
    match k {
        K::SingBoxRemote => "singbox_remote".to_string(),
        K::SingBoxLocal => "singbox_local".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input for saving local override (full replacement).
///
/// 场景模板已移除：`applied_templates` / `custom_templates` 不再是输入契约，
/// 前端若仍附带同名字段会被 serde 忽略。
#[derive(Debug, Deserialize)]
pub struct SaveLocalOverrideInput {
    pub singbox: CoreLocalOverrideInput,
    /// Full replacement of the custom rule set segment (semantics identical
    /// to `rules`). Uses the shared model type so the manual content / remote
    /// url+format round-trips unchanged.
    #[serde(default)]
    pub custom_rule_sets: Vec<pp_client::local_override::CustomRuleSet>,
}

#[derive(Debug, Deserialize)]
pub struct CoreLocalOverrideInput {
    pub rules: Vec<LocalRuleInput>,
    pub rule_sets: Vec<LocalRuleSetRefInput>,
    /// Compatibility field: the model master switch was removed. Accepted for
    /// the frontend contract but ignored on conversion (rules / rule sets carry
    /// their own `enabled`).
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

#[inline]
#[must_use]
pub(crate) const fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Validation

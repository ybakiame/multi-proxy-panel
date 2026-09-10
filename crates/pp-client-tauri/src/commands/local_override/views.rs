//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{
    AppliedTemplate, CoreLocalOverride, CustomRuleSetSource, LocalOverride, LocalRule,
    MarketManager, MarketSource, RuleSetFormat, RuleSetManager,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// View types (shell layer, for frontend contract)
// ---------------------------------------------------------------------------

/// Full local override view (returned by `local_override_get`).
///
/// 自「废弃内置规则集订阅」起不再输出 `rule_set_subscriptions` 段；自定义规则集
/// 状态（含 `cached` / `last_updated`）由 `custom_rule_sets` 段承载，前端统一走本
/// 命令消费。自「市场改为用户自定义源」起额外输出 `market_sources`（源列表 +
/// 条目数），市场条目本体走 `local_override_market_entries` 读缓存。
#[derive(Debug, Clone, Serialize)]
pub struct LocalOverrideView {
    pub singbox: CoreLocalOverrideView,
    pub applied_templates: Vec<AppliedTemplateView>,
    pub custom_rule_sets: Vec<CustomRuleSetView>,
    pub custom_templates: Vec<CustomTemplateView>,
    pub market_sources: Vec<MarketSourceView>,
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
///
/// 自「规则集移除 enabled」起不再输出 `enabled`（纯资源，旧文件中的该字段由
/// serde 忽略，不再出现在视图契约里）。
#[derive(Debug, Clone, Serialize)]
pub struct CustomRuleSetView {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub source: CustomRuleSetSource,
    pub last_updated: u64,
    pub cached: bool,
}

/// User-defined scenario template view.
///
/// `rules` 是**规则 ID 引用列表**（非快照）；`invalid_count` 由服务端按
/// 「引用 ID 不存在 或 对应规则 disabled」计算（模板卡据此显示失效数）。
#[derive(Debug, Clone, Serialize)]
pub struct CustomTemplateView {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub rules: Vec<String>,
    /// Number of currently invalid (missing / disabled) rule references.
    pub invalid_count: usize,
    pub created_at: u64,
}

/// User-added market source view (from `local_override_get`).
///
/// `entry_count` 为本地缓存目录中的有效条目数（纯读缓存，不拉网络）。
#[derive(Debug, Clone, Serialize)]
pub struct MarketSourceView {
    pub id: String,
    pub name: String,
    pub url: String,
    /// Last successful fetch timestamp (Unix seconds; 0 = never).
    pub last_fetched: u64,
    /// Valid entries in the cached catalog.
    pub entry_count: usize,
}

/// Market entry view (from `local_override_market_entries`).
///
/// 合并全部源缓存条目，`source_id` / `source_name` 标注来源。
#[derive(Debug, Clone, Serialize)]
pub struct MarketEntryView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub format: RuleSetFormat,
    pub url: String,
    pub source_id: String,
    pub source_name: String,
}

// ---------------------------------------------------------------------------
// Conversions (pp-client types → View types)
// ---------------------------------------------------------------------------

impl LocalOverrideView {
    pub(crate) fn from_model(
        model: &LocalOverride,
        manager: &RuleSetManager,
        market: &MarketManager,
    ) -> Self {
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
                .map(|tpl| CustomTemplateView::from_model(tpl, model))
                .collect(),
            market_sources: model
                .market_sources
                .iter()
                .map(|src| MarketSourceView::from_model(src, market))
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
            last_updated: model.last_updated,
            cached: manager.has_custom_rule_set_file(model),
        }
    }
}

impl CustomTemplateView {
    pub(crate) fn from_model(
        model: &pp_client::local_override::CustomTemplate,
        ovr: &LocalOverride,
    ) -> Self {
        let invalid_count = pp_client::local_override::template_invalid_refs(ovr, model).len();
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            desc: model.desc.clone(),
            rules: model.rules.clone(),
            invalid_count,
            created_at: model.created_at,
        }
    }
}

impl MarketSourceView {
    pub(crate) fn from_model(model: &MarketSource, market: &MarketManager) -> Self {
        Self {
            id: model.id.clone(),
            name: model.name.clone(),
            url: model.url.clone(),
            last_fetched: model.last_fetched,
            entry_count: market.cached_entry_count(&model.id),
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
    /// `rules`). Templates carry rule **ID references** (`rules: Vec<String>`).
    #[serde(default)]
    pub custom_templates: Vec<CustomTemplateInput>,
}

/// Input for one custom scenario template (rule ID reference list).
#[derive(Debug, Deserialize)]
pub struct CustomTemplateInput {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub rules: Vec<String>,
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

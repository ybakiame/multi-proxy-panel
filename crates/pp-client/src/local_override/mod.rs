//! Local Override layer for client-side rule management (sing-box only).
//!
//! ADR-0002: Client rule management redesign (local override layer + rule cards).
//!
//! 自「废弃内置规则集订阅与内置场景模板」起只保留用户自控模型：规则卡片 +
//! [`CustomRuleSet`] + [`CustomTemplate`]；`rule_set_subscriptions` 段仅作旧文件
//! serde 兼容（load 内迁移清空）。
//!
//! Module structure:
//! - `schema` — type definitions (`LocalOverride`, `LocalRule`, `RuleMatchType`, etc.)
//! - `store` — `LocalOverrideStore` for `local_override.json` read/write + 存量迁移
//! - `template` — user-defined custom templates (apply / revert)
//! - `ruleset` — custom rule set download, cache, and file sync
//! - `singbox` — sing-box config injection (`apply_singbox_local_override`)

pub mod ruleset;
pub mod schema;
pub mod singbox;
pub mod store;
pub mod template;

pub use ruleset::*;
pub use schema::*;
pub use singbox::*;
pub use store::*;
pub use template::*;

use serde_json::Value;

/// Apply local override to a composed core config.
///
/// No-op if the config is not a JSON object.
///
/// # Panics
///
/// Never panics; errors are logged as warnings.
pub fn apply_local_override(config: &mut Value, ovr: &CoreLocalOverride) {
    singbox::apply_singbox_local_override(config, ovr)
}

/// Inject local override into a composed core config; failure is logged as a warning only.
///
/// Extracted from `ClientState::start()` so the preview command (and any other
/// caller) can reuse the exact same injection, eliminating a double source of
/// truth (ADR-0005 §3.4).
///
/// - Missing or corrupted `local_override.json` → treated as empty config (no-op).
/// - Injection failure → warning log, does not block startup.
///
/// 自「场景模板改为规则引用 + 应用激活」起，`singbox.rules` 的注入条件为
/// **`enabled && id ∈ 激活集合`**：激活集合 = ∪（已应用模板各自引用列表，
/// 见 [`active_rule_ids`]）。未被任何已应用模板引用的规则不注入启动配置
/// （模板的应用/撤销=场景开关，不复制规则）。规则集注入（[`apply_custom_rule_sets`]）
/// 随之只看到被注入的规则，规则集条目也仅由这些规则引用。
pub fn inject_local_override_warn_only(data_dir: &std::path::Path, config: &mut Value) {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let ovr = match store.load() {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to load local_override.json, skipping injection"
            );
            return;
        }
    };

    let manager = RuleSetManager::new(data_dir.to_path_buf());

    // 激活规则 ID 集合 → 只注入已应用模板引用的启用规则。
    let active = active_rule_ids(&ovr);
    let mut core = ovr.singbox.clone();
    core.rules.retain(|r| r.enabled && active.contains(&r.id));

    // 本地规则卡片前插（rule_set 引用由用户规则卡片定义并校验到 custom tag）。
    apply_local_override(config, &core);

    // 注入用户自定义 rule sets（remote cache / manual JSON）为 local rule_set 条目；
    // 仅注入「被注入的 rule_set 规则引用」的 tag（见 apply_custom_rule_sets 语义）。
    apply_custom_rule_sets(config, &manager, &core.rules, &ovr.custom_rule_sets);
}

//! Local Override layer for client-side rule management (sing-box only).
//!
//! ADR-0002: Client rule management redesign (local override layer + rule cards).
//!
//! 自「移除场景模板」起只保留用户自控模型：规则卡片 + [`CustomRuleSet`]；
//! `rule_set_subscriptions` / `applied_templates` / `custom_templates` 段仅作旧文件
//! serde 兼容（load 内迁移/清空）。
//!
//! Module structure:
//! - `schema` — type definitions (`LocalOverride`, `LocalRule`, `RuleMatchType`, etc.)
//! - `store` — `LocalOverrideStore` for `local_override.json` read/write + 存量迁移
//! - `ruleset` — custom rule set download, cache, and file sync
//! - `singbox` — sing-box config injection (`apply_singbox_local_override`)

pub mod ruleset;
pub mod schema;
pub mod singbox;
pub mod store;

pub use ruleset::*;
pub use schema::*;
pub use singbox::*;
pub use store::*;

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
/// 自「移除场景模板」起，`singbox.rules` 的注入条件为 **`enabled`**：所有启用的
/// 规则卡片全部注入（不再按模板引用过滤）。规则集注入
/// （[`apply_custom_rule_sets`]）随之只看到被注入的规则，规则集条目也仅由这些
/// 规则引用。
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

    // 注入全部启用的本地规则（场景模板移除后不再有引用过滤）。
    let mut core = ovr.singbox.clone();
    core.rules.retain(|r| r.enabled);

    // 本地规则卡片前插（rule_set 引用由用户规则卡片定义并校验到 custom tag）。
    apply_local_override(config, &core);

    // 注入用户自定义 rule sets（remote cache / manual JSON）为 local rule_set 条目；
    // 仅注入「被注入的 rule_set 规则引用」的 tag（见 apply_custom_rule_sets 语义）。
    apply_custom_rule_sets(config, &manager, &core.rules, &ovr.custom_rule_sets);
}

/// 启动前补齐「backing 文件缺失的 remote 规则集」（best-effort，全部失败不阻断启动）。
///
/// 背景：社区/市场规则集是 remote 资源，添加后需「立即更新」才有本地内容；用户未下载时
/// 引用它的规则会在规则集物化阶段被剥离（`ruleset_manager`），表现为「规则加了但不生
/// 效」。启动时统一补下载一次（逐个尝试，单条失败仅告警），让「添加即用」成立。
pub async fn download_missing_rule_sets_warn_only(data_dir: &std::path::Path) {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let Ok(ovr) = store.load() else {
        return;
    };
    let manager = RuleSetManager::new(data_dir.to_path_buf());
    for rs in &ovr.custom_rule_sets {
        if !matches!(rs.source, CustomRuleSetSource::Remote { .. })
            || manager.has_custom_rule_set_file(rs)
        {
            continue;
        }
        if let Err(e) = manager.download_custom_rule_set(rs).await {
            tracing::warn!(
                tag = %rs.tag,
                error = %e,
                "规则集启动补下载失败（引用它的规则本次启动不生效）"
            );
        } else {
            tracing::info!(tag = %rs.tag, "规则集启动补下载完成");
        }
    }
}

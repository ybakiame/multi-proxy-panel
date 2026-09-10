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
//! - `market` — user-defined market source fetch, cache, and cleanup
//! - `singbox` — sing-box config injection (`apply_singbox_local_override`)

pub mod market;
pub mod ruleset;
pub mod schema;
pub mod singbox;
pub mod store;
pub mod template;

pub use market::*;
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

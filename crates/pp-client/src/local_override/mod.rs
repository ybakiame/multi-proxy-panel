//! Local Override layer for client-side rule management (sing-box only).
//!
//! ADR-0002: Client rule management redesign (local override layer + rule cards + rule set subscriptions).
//!
//! Module structure:
//! - `schema` — type definitions (`LocalOverride`, `LocalRule`, `RuleMatchType`, etc.)
//! - `store` — `LocalOverrideStore` for `local_override.json` read/write
//! - `template` — scenario templates (return-china, overseas, ad-filter)
//! - `ruleset` — rule set download, cache, and subscription management
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

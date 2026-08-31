//! Local Override layer for client-side rule management.
//!
//! ADR-0002: Client rule management redesign (local override layer + rule cards + rule set subscriptions).
//!
//! Module structure:
//! - `schema` — type definitions (`LocalOverride`, `LocalRule`, `RuleMatchType`, etc.)
//! - `store` — `LocalOverrideStore` for `local_override.json` read/write
//! - `template` — scenario templates (return-china, overseas, ad-filter)
//! - `ruleset` — rule set download, cache, and subscription management
//! - `singbox` — sing-box config injection (`apply_singbox_local_override`)
//! - `mihomo` — mihomo config injection (`apply_mihomo_local_override`)

pub mod mihomo;
pub mod ruleset;
pub mod schema;
pub mod singbox;
pub mod store;
pub mod template;

pub use mihomo::*;
pub use ruleset::*;
pub use schema::*;
pub use singbox::*;
pub use store::*;
pub use template::*;

use pp_common::CoreType;
use serde_json::Value;

/// Apply local override to a composed core config.
///
/// Dispatches to the appropriate core-specific injector.
/// No-op if the config is not a JSON object.
///
/// # Panics
///
/// Never panics; errors are logged as warnings.
pub fn apply_local_override(config: &mut Value, core_type: CoreType, ovr: &CoreLocalOverride) {
    match core_type {
        CoreType::SingBox => singbox::apply_singbox_local_override(config, ovr),
        CoreType::Mihomo => mihomo::apply_mihomo_local_override(config, ovr),
    }
}

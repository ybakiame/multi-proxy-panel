//! Config slices (ADR-0005 P0/P1): structured DNS + custom outbound + experimental slices.
//!
//! A config slice is a small, user-editable projection of a sing-box top-level
//! section, stored at `data_dir/config_slices.json`. Each slice has its own
//! `enabled` switch; disabled slices are never injected.
//!
//! Module structure:
//! - [`dns`] — DNS slice schema (`DnsSlice` / `DnsServer` / `DnsRule`, …)
//! - [`dns_render`] — pure DNS slice rendering (`render_dns`)
//! - [`outbound`] — custom outbound slice schema (`CustomOutbound`, protocols, `outbound_tag`)
//! - [`experimental`] — experimental slice schema (`ExperimentalSlice` / `CacheFileSlice`)
//! - [`schema`] — [`ConfigSlices`] container + cross-slice validation
//! - [`store`] — [`ConfigSlicesStore`] for `config_slices.json` read/write
//! - [`apply`] — `apply_config_slices` + pure `render_outbound` / `render_cache_file`
//!
//! The slice layer is injected as the **first** step of the client config
//! pipeline (before profile YAML/JS overrides, ADR-0005 §3.2). This module only
//! builds the module and its tests; wiring into the pipeline is a later phase.
//!
//! Gate: the custom outbound and experimental slices are additionally gated by
//! the local-override master switch (`local_override.json` `singbox.enabled`);
//! the DNS slice stays ungated (required config). See
//! [`apply::apply_config_slices`].
//!
//! Rendering targets **sing-box >= 1.14.0**; compatibility with older releases
//! is intentionally not maintained. Fields emitted here are verified against the
//! 1.14 configuration docs: vmess `alter_id` / `security`, vless `flow`, the
//! V2Ray transport kinds (`ws` / `grpc` / `http` / `httpupgrade`), hysteria2
//! `obfs`, the selector / urltest group outbounds, and the 1.12+ type-based DNS
//! server format are all still valid. The DNS slice additionally covers the
//! `fakeip` server type (`inet4_range` / `inet6_range`), the `query_type` match
//! field, and the `route` / `predefined` / `reject` rule actions.

use serde_json::Value;

pub mod apply;
pub mod dns;
pub mod dns_render;
pub mod experimental;
pub mod outbound;
pub mod schema;
pub mod store;

pub use apply::*;
pub use dns::*;
pub use dns_render::*;
pub use experimental::*;
pub use outbound::*;
pub use schema::*;
pub use store::*;

/// serde default for booleans that default to `true`.
pub(crate) const fn default_true() -> bool {
    true
}

/// Short helper for a JSON string value, shared by the slice renderers.
pub(crate) fn str_value(value: &str) -> Value {
    Value::String(value.to_string())
}

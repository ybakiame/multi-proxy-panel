//! Config slices (ADR-0005 P0/P1): structured DNS + custom outbound + experimental slices.
//!
//! A config slice is a small, user-editable projection of a sing-box top-level
//! section, stored at `data_dir/config_slices.json`. Each slice has its own
//! `enabled` switch; disabled slices are never injected.
//!
//! Module structure:
//! - [`dns`] — DNS slice schema (`DnsSlice` / `DnsServer` / `DnsRule`, …)
//! - [`outbound`] — custom outbound slice schema (`CustomOutbound`, protocols, `outbound_tag`)
//! - [`experimental`] — experimental slice schema (`ExperimentalSlice` / `CacheFileSlice`)
//! - [`schema`] — [`ConfigSlices`] container + cross-slice validation
//! - [`store`] — [`ConfigSlicesStore`] for `config_slices.json` read/write
//! - [`apply`] — `apply_config_slices` + pure `render_dns` / `render_outbound` / `render_cache_file`
//!
//! The slice layer is injected as the **first** step of the client config
//! pipeline (before profile YAML/JS overrides, ADR-0005 §3.2). This module only
//! builds the module and its tests; wiring into the pipeline is a later phase.
//!
//! Rendering targets **sing-box >= 1.14.0**; compatibility with older releases
//! is intentionally not maintained. Fields emitted here are verified against the
//! 1.14 configuration docs: vmess `alter_id` / `security`, vless `flow`, the
//! V2Ray transport kinds (`ws` / `grpc` / `http` / `httpupgrade`), hysteria2
//! `obfs`, the selector / urltest group outbounds, and the 1.12+ type-based DNS
//! server format are all still valid.

pub mod apply;
pub mod dns;
pub mod experimental;
pub mod outbound;
pub mod schema;
pub mod store;

pub use apply::*;
pub use dns::*;
pub use experimental::*;
pub use outbound::*;
pub use schema::*;
pub use store::*;

/// serde default for booleans that default to `true`.
pub(crate) const fn default_true() -> bool {
    true
}

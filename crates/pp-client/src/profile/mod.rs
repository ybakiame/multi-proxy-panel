//! Profile layer: subscriptions only fetch nodes, local templates generate rules,
//! supporting YAML deep-merge override + JS override.
//!
//! Aligned with Clash Verge's Merge + Script:
//! - Subscription content is only used to extract proxy nodes (sing-box `outbounds` leaves),
//!   the actual running client config is generated from local templates,
//!   avoiding subscription-bundled groups / rules / routing overriding local settings.
//! - YAML override ([`apply_yaml_override`]) deep-merges per RFC 7386: objects merge
//!   recursively, arrays and scalars are replaced entirely.
//! - JS override ([`apply_js_override`]) is a synchronous pure-function mode
//!   `function main(config){...; return config}`, driven by pp-script [`ScriptWorker`]
//!   (dedicated thread + current_thread runtime, `Send` future), host uses [`DenyHttpExecutor`]
//!   to deny all network access, and memory storage for persistence (no disk writes),
//!   i.e. "no network / no storage permissions".
//!
//! Assembly entrypoint [`build_core_config_v2`] (remote + local overlay; old signature
//! [`build_core_config`] kept for compatibility): extract nodes → local template →
//! ⓪ config slices → remote YAML → local YAML → remote JS → local JS.
//! Remote override URLs are fetched via [`resolve_remote_overrides`] and cached for fallback.
//! inbounds and MITM chain are not handled in this layer, still injected by [`crate::state`]
//! through [`crate::core_config`]'s `compose_*`.

mod overrides;
#[cfg(test)]
mod rule_set_tests;
mod store;
#[cfg(test)]
mod tests;

pub use overrides::*;
pub use store::*;

use pp_common::{PanelError, PanelResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::config_slices::{ConfigSlices, apply_config_slices};
use crate::core_config::{
    OUTBOUND_KIND_BLOCK, OUTBOUND_KIND_DIRECT, OUTBOUND_KIND_SELECTOR, OUTBOUND_KIND_URLTEST,
    OUTBOUND_TAG_AUTO, OUTBOUND_TAG_BLOCK, OUTBOUND_TAG_DIRECT, OUTBOUND_TAG_PROXY,
};

/// Profile override config: empty string = disabled.
///
/// As the payload of the legacy single-file storage [`ProfileStore`] (only for compatibility
/// with legacy callers).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOverrides {
    /// YAML deep-merge override (RFC 7386 style; empty string = disabled).
    pub yaml_override: String,
    /// JS override (synchronous pure function `function main(config){...; return config}`;
    /// empty string = disabled).
    pub js_override: String,
}

/// Effective overrides after remote + local overlay (remote as base, local overrides).
///
/// Produced by [`resolve_remote_overrides`]: `remote_*` is the content fetched/cached from
/// remote URLs, `local_*` is the Profile local override. Consumed by [`build_core_config_v2`]:
/// YAML stage applies remote first then local (deep merge naturally satisfies local override);
/// JS stage remote `main` executes first, local `main` executes second (chained, local sees
/// remote result).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveOverrides {
    /// Remote YAML override content (empty string = none).
    pub remote_yaml: String,
    /// Local YAML override content (empty string = none).
    pub local_yaml: String,
    /// Remote JS override source (empty string = none).
    pub remote_js: String,
    /// Local JS override source (empty string = none).
    pub local_js: String,
}

/// A Profile template (pure association model): multiple templates can be maintained,
/// the runtime override = the template associated with the currently selected subscription,
/// the template itself does not hold an enabled state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Template unique identifier (application-layer generated Uuid v4).
    pub id: Uuid,
    /// Template name (unique within storage, duplicate names error).
    pub name: String,
    /// YAML deep-merge override (RFC 7386 style; empty string = disabled).
    pub yaml_override: String,
    /// JS override (synchronous pure function `function main(config){...; return config}`;
    /// empty string = disabled).
    pub js_override: String,
    /// Remote YAML override URL (http/https; fetched at startup, fallback to cache on failure;
    /// `None` = not configured).
    #[serde(default)]
    pub yaml_url: Option<String>,
    /// Remote JS override URL (http/https; fetched at startup, fallback to cache on failure;
    /// `None` = not configured).
    #[serde(default)]
    pub js_url: Option<String>,
}

/// sing-box outbound leaf protocol types (excluding groups and built-in types).
const SINGBOX_LEAF_TYPES: &[&str] = &[
    "vless",
    "vmess",
    "trojan",
    "shadowsocks",
    "shadowsocksr",
    "hysteria",
    "hysteria2",
    "tuic",
    "anytls",
    "wireguard",
    "ssh",
    "http",
    "socks",
];

/// Extract leaf nodes from sing-box subscription config (leaf types in outbounds;
/// excluding selector / urltest / direct / block / dns etc.). Tag deduplication:
/// duplicate names appended with `-2` / `-3`.
pub fn extract_nodes_singbox(sub: &Value) -> Vec<Value> {
    let leaves: Vec<Value> = sub
        .get("outbounds")
        .and_then(|o| o.as_array())
        .map(|outbounds| {
            outbounds
                .iter()
                .filter(|o| {
                    o.get("type")
                        .and_then(|t| t.as_str())
                        .is_some_and(|t| SINGBOX_LEAF_TYPES.contains(&t))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    dedup_names(leaves, "tag")
}

/// Extract `proxies` nodes from clash YAML subscription text (ClashYaml subscriptions are
/// converted to sing-box nodes via [`crate::node_convert::mihomo_to_singbox`]). Name
/// deduplication: duplicate names appended with `-2` / `-3`.
pub(crate) fn extract_clash_proxies(sub_yaml: &str) -> PanelResult<Vec<Value>> {
    let value: Value = serde_yaml::from_str(sub_yaml)
        .map_err(|e| PanelError::Client(format!("invalid clash config in subscription: {e}")))?;
    let proxies = value
        .get("proxies")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(dedup_names(proxies, "name"))
}

/// Deduplicate by `key` (sing-box uses `tag`, clash uses `name`): duplicate nodes are
/// appended with `-2` / `-3` … until unique; nodes missing key or with empty key are skipped
/// (group references cannot address empty tags, skipping is safer).
fn dedup_names(nodes: Vec<Value>, key: &str) -> Vec<Value> {
    let mut used = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(nodes.len());
    for mut node in nodes {
        let name = match node.get(key).and_then(|n| n.as_str()) {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => continue,
        };
        let mut candidate = name.clone();
        let mut n = 2u32;
        while used.contains(&candidate) {
            candidate = format!("{name}-{n}");
            n += 1;
        }
        used.insert(candidate.clone());
        node[key] = Value::String(candidate);
        out.push(node);
    }
    out
}

/// sing-box local template (CN-split baseline, aligned with the GUI.for.SingBox default
/// profile): log + CN-split DNS + all leaf nodes + `proxy` (select, default `auto`) /
/// `auto` (url-test) groups + `direct` / `block` + CN-split routing.
///
/// `route.rules` carries the CN-split baseline (private / CN → `direct`, non-CN → main
/// selector) and `route.rule_set` registers the five remote MetaCubeX jsDelivr rule sets.
/// `compose_singbox_config`, local-override and panel-feature rules all **prepend**, so user
/// rules keep priority over the baseline and the baseline acts as the final split before
/// `route.final`.
///
/// `route.default_domain_resolver` is directly embedded, ensuring the template is natively
/// valid in sing-box 1.12+ (required when `dns.servers` exists). When no leaf nodes exist, the
/// `auto` group falls back to built-in `direct` to keep the config valid.
///
/// DNS split (aligned with GUI.for.SingBox):
/// - `local` = DoH (`https` 223.5.5.5:443), IP literal so no `domain_resolver` is needed; it
///   has no `detour` (default direct dial) and serves `route.default_domain_resolver`.
/// - `remote` = DoH (`https` 8.8.8.8:443), IP literal, `detour` = main selector (read from the
///   generated outbounds, never hardcoded) so foreign resolution goes through the proxy. DoH on
///   443 shares the well-worn HTTPS path through the proxy (bare 853 is interfered with on some
///   node egress networks), aligning with the reference templates.
/// - `rules` = [`crate::core_config::cn_baseline_dns_rules`]: `clash_mode` direct/global →
///   local/remote, `geosite-cn` → local; `final = remote` resolves the rest.
/// - `reverse_mapping = true`: after hijacked DNS resolves a domain, sing-box maps the
///   returned IP back to the domain for routing/records. Without it TUN connections arrive as
///   bare IPs and Clash API `metadata.host` stays empty whenever payload sniffing cannot
///   recover the domain (QUIC, non-TLS/HTTP, IP-literal Host).
///
/// Startup note: sing-box downloads remote rule sets synchronously at startup and **fails to
/// start** when a URL is unreachable (verified against sing-box 1.14). The jsDelivr mirror is
/// directly reachable in CN; `experimental.cache_file.store_dns` (or the deprecated
/// `store_rdrc`) is the only offline fallback and is not enabled by the baseline.
pub fn singbox_template(nodes: &[Value]) -> Value {
    let tags: Vec<String> = nodes
        .iter()
        .filter_map(|n| n["tag"].as_str().map(String::from))
        .collect();
    let mut auto_outbounds: Vec<Value> = tags.iter().cloned().map(Value::String).collect();
    if auto_outbounds.is_empty() {
        auto_outbounds.push(Value::String(OUTBOUND_TAG_DIRECT.to_string()));
    }
    let mut proxy_outbounds = vec![Value::String(OUTBOUND_TAG_AUTO.to_string())];
    proxy_outbounds.extend(tags.iter().cloned().map(Value::String));

    let mut outbounds = nodes.to_vec();
    outbounds.push(json!({
        "type": OUTBOUND_KIND_SELECTOR,
        "tag": OUTBOUND_TAG_PROXY,
        "outbounds": proxy_outbounds,
        "default": OUTBOUND_TAG_AUTO
    }));
    outbounds.push(json!({
        "type": OUTBOUND_KIND_URLTEST,
        "tag": OUTBOUND_TAG_AUTO,
        "outbounds": auto_outbounds,
        "url": "https://www.gstatic.com/generate_204",
        "interval": "5m"
    }));
    outbounds.push(json!({ "type": OUTBOUND_KIND_DIRECT, "tag": OUTBOUND_TAG_DIRECT }));
    outbounds.push(json!({ "type": OUTBOUND_KIND_BLOCK, "tag": OUTBOUND_TAG_BLOCK }));

    // Main selector tag: read from the generated outbounds (never hardcoded) so the CN-split
    // baseline and the remote DNS detour follow the template's actual group tag.
    let proxy_tag = outbounds
        .iter()
        .find(|o| o.get("type").and_then(Value::as_str) == Some(OUTBOUND_KIND_SELECTOR))
        .and_then(|o| o.get("tag").and_then(Value::as_str))
        .unwrap_or(OUTBOUND_TAG_PROXY)
        .to_string();

    let mut cfg = json!({
        "log": { "level": "info" },
        "dns": {
            "servers": [
                { "tag": "local", "type": "https", "server": "223.5.5.5", "server_port": 443 },
                {
                    "tag": "remote",
                    "type": "https",
                    "server": "8.8.8.8",
                    "server_port": 443,
                    "detour": proxy_tag.clone()
                }
            ],
            "rules": crate::core_config::cn_baseline_dns_rules(),
            "final": "remote",
            "reverse_mapping": true,
            "strategy": "prefer_ipv4"
        },
        "route": {
            "rules": crate::core_config::cn_baseline_route_rules(&proxy_tag),
            "final": proxy_tag,
            "auto_detect_interface": true,
            "default_domain_resolver": { "server": "local" }
        }
    });
    cfg["outbounds"] = Value::Array(outbounds);
    // Register the five remote CN-split rule sets (idempotent by tag).
    if let Some(route) = cfg["route"].as_object_mut() {
        crate::core_config::ensure_cn_rule_sets(route);
    }
    cfg
}

/// Assembly (v2, supports remote override overlay): extract nodes → local template →
/// ⓪ config slices → remote YAML → local YAML → remote JS → local JS → return
/// sing-box-usable config.
///
/// `sub` is the sing-box subscription config (nodes extracted from `outbounds`; ClashYaml
/// subscriptions have already been converted to sing-box nodes at fetch time).
///
/// `slices` are the structured local config slices (ADR-0005 §3.2), injected **before**
/// the YAML/JS overrides so the override escape hatch always wins (D2). The caller owns
/// loading them (no IO in this function).
///
/// Overlay semantics: remote as base, local overrides — YAML stage applies remote first then
/// local (two deep merges naturally satisfy local override); JS stage remote `main` executes
/// first, local `main` executes second (chained, local sees remote result). inbounds and MITM
/// chain are not handled in this layer, injected by `state` calling `compose_*`.
pub async fn build_core_config_v2(
    sub: &Value,
    effective: &EffectiveOverrides,
    slices: &ConfigSlices,
) -> PanelResult<Value> {
    let mut config = singbox_template(&extract_nodes_singbox(sub));
    // ⓪ Slice layer: before the profile overrides so YAML/JS wins on field conflicts (ADR-0005 D2).
    let report = apply_config_slices(&mut config, slices)?;
    if report.dns_applied || !report.outbound_tags.is_empty() {
        tracing::info!(
            dns_applied = report.dns_applied,
            outbound_tags = ?report.outbound_tags,
            renamed_outbounds = ?report.renamed_outbounds,
            "config slices applied"
        );
    }
    // YAML stage: remote as base, local overlay (two applications naturally satisfy local override).
    let merged = apply_yaml_override(config, &effective.remote_yaml)?;
    let merged = apply_yaml_override(merged, &effective.local_yaml)?;
    // JS stage: remote main first, local main second (IIFE isolates name conflicts, chained call).
    let js = compose_js_chain(&effective.remote_js, &effective.local_js);
    if js.is_empty() {
        Ok(merged)
    } else {
        apply_js_override(merged, &js).await
    }
}

/// Assembly (old signature compatibility): extract nodes → local template → YAML override →
/// JS override → return core-usable config.
///
/// Only local overrides (no remote URLs) and no config slices ([`ConfigSlices::default`]);
/// for remote override / slice scenarios please use [`build_core_config_v2`]. inbounds and
/// MITM chain are not handled in this layer, injected by `state` calling `compose_*`.
pub async fn build_core_config(sub: &Value, overrides: &ProfileOverrides) -> PanelResult<Value> {
    build_core_config_v2(
        sub,
        &EffectiveOverrides {
            remote_yaml: String::new(),
            local_yaml: overrides.yaml_override.clone(),
            remote_js: String::new(),
            local_js: overrides.js_override.clone(),
        },
        &ConfigSlices::default(),
    )
    .await
}

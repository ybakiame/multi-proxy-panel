//! Profile layer: subscriptions only fetch nodes, local templates generate rules,
//! supporting YAML deep-merge override + JS override.
//!
//! Aligned with Clash Verge's Merge + Script:
//! - Subscription content is only used to extract proxy nodes (sing-box `outbounds` leaves /
//!   mihomo `proxies`), the actual running client config is generated from local templates,
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
//! remote YAML → local YAML → remote JS → local JS.
//! Remote override URLs are fetched via [`resolve_remote_overrides`] and cached for fallback.
//! inbounds and MITM chain are not handled in this layer, still injected by [`crate::state`]
//! through [`crate::core_config`]'s `compose_*`.

mod overrides;
mod store;
#[cfg(test)]
mod tests;

pub use overrides::*;
pub use store::*;

use pp_common::{CoreType, PanelError, PanelResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

/// Subscription content (two core subscription formats kept as-is), for [`build_core_config`]
/// to extract nodes.
#[derive(Debug, Clone)]
pub enum SubContent {
    /// sing-box JSON subscription config.
    SingBox(Value),
    /// clash/mihomo YAML subscription raw text.
    Mihomo(String),
}

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

/// A Profile template (pure association model): multiple templates can be maintained for the
/// same core type ([`CoreType`]), the runtime override = the template associated with the
/// currently selected subscription, the template itself does not hold an enabled state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Template unique identifier (application-layer generated Uuid v4).
    pub id: Uuid,
    /// Template name (unique within storage, duplicate names error).
    pub name: String,
    /// Target core type (sing-box / mihomo).
    pub core_type: CoreType,
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

/// Extract `proxies` nodes from clash/mihomo subscription YAML. Name deduplication:
/// duplicate names appended with `-2` / `-3`.
pub fn extract_nodes_mihomo(sub_yaml: &str) -> PanelResult<Vec<Value>> {
    let value: Value = serde_yaml::from_str(sub_yaml)
        .map_err(|e| PanelError::Client(format!("invalid clash config in subscription: {e}")))?;
    let proxies = value
        .get("proxies")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(dedup_names(proxies, "name"))
}

/// Deduplicate by `key` (sing-box uses `tag`, mihomo uses `name`): duplicate nodes are
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

/// sing-box local template: log + dns (local UDP + remote DoH) + all leaf nodes + `proxy`
/// (select, default `auto`) / `auto` (url-test) groups + `direct` / `block` + empty routing.
///
/// `route.rules` is an empty array (compatible with `compose_singbox_config`'s MITM rule
/// prepending); `route.default_domain_resolver` is directly embedded, ensuring the template
/// is natively valid in sing-box 1.12+ (required when `dns.servers` exists). When no leaf
/// nodes exist, the `auto` group falls back to built-in `direct` to keep the config valid.
pub fn singbox_template(nodes: &[Value]) -> Value {
    let tags: Vec<String> = nodes
        .iter()
        .filter_map(|n| n["tag"].as_str().map(String::from))
        .collect();
    let mut auto_outbounds: Vec<Value> = tags.iter().cloned().map(Value::String).collect();
    if auto_outbounds.is_empty() {
        auto_outbounds.push(Value::String("direct".to_string()));
    }
    let mut proxy_outbounds = vec![Value::String("auto".to_string())];
    proxy_outbounds.extend(tags.iter().cloned().map(Value::String));

    let mut cfg = json!({
        "log": { "level": "info" },
        "dns": {
            "servers": [
                { "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 },
                { "tag": "remote", "type": "https", "server": "8.8.8.8", "server_port": 443 }
            ],
            "strategy": "prefer_ipv4"
        },
        "route": {
            "rules": [],
            "final": "proxy",
            "auto_detect_interface": true,
            "default_domain_resolver": { "server": "local" }
        }
    });
    let mut outbounds = nodes.to_vec();
    outbounds.push(json!({
        "type": "selector",
        "tag": "proxy",
        "outbounds": proxy_outbounds,
        "default": "auto"
    }));
    outbounds.push(json!({
        "type": "urltest",
        "tag": "auto",
        "outbounds": auto_outbounds,
        "url": "https://www.gstatic.com/generate_204",
        "interval": "5m"
    }));
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));
    outbounds.push(json!({ "type": "block", "tag": "block" }));
    cfg["outbounds"] = Value::Array(outbounds);
    cfg
}

/// mihomo local template: dns + all proxies + `proxy` (select, includes `auto`) / `auto`
/// (url-test, interval 300) groups + `MATCH,proxy` rule.
///
/// When no leaf nodes exist, the `auto` group falls back to built-in `DIRECT` to keep the
/// config valid.
pub fn mihomo_template(nodes: &[Value]) -> Value {
    let names: Vec<String> = nodes
        .iter()
        .filter_map(|n| n["name"].as_str().map(String::from))
        .collect();
    let mut auto_proxies: Vec<Value> = names.iter().cloned().map(Value::String).collect();
    if auto_proxies.is_empty() {
        auto_proxies.push(Value::String("DIRECT".to_string()));
    }
    let mut proxy_proxies = vec![Value::String("auto".to_string())];
    proxy_proxies.extend(names.iter().cloned().map(Value::String));

    let mut cfg = json!({
        "dns": {
            "enable": true,
            "nameserver": ["223.5.5.5"],
            "fallback": ["dns.google"]
        },
        "proxy-groups": [
            { "name": "proxy", "type": "select", "proxies": proxy_proxies },
            {
                "name": "auto",
                "type": "url-test",
                "proxies": auto_proxies,
                "url": "https://www.gstatic.com/generate_204",
                "interval": 300
            }
        ],
        "rules": ["MATCH,proxy"]
    });
    cfg["proxies"] = Value::Array(nodes.to_vec());
    cfg
}

/// Assembly (v2, supports remote override overlay): extract nodes → local template →
/// remote YAML → local YAML → remote JS → local JS → return core-usable config.
///
/// Overlay semantics: remote as base, local overrides — YAML stage applies remote first then
/// local (two deep merges naturally satisfy local override); JS stage remote `main` executes
/// first, local `main` executes second (chained, local sees remote result). inbounds and MITM
/// chain are not handled in this layer, injected by `state` calling `compose_*`.
pub async fn build_core_config_v2(
    core_type: CoreType,
    sub_content: &SubContent,
    effective: &EffectiveOverrides,
) -> PanelResult<Value> {
    let config = match (core_type, sub_content) {
        (CoreType::SingBox, SubContent::SingBox(sub)) => {
            singbox_template(&extract_nodes_singbox(sub))
        }
        (CoreType::Mihomo, SubContent::Mihomo(yaml)) => {
            mihomo_template(&extract_nodes_mihomo(yaml)?)
        }
        _ => {
            return Err(PanelError::Client(
                "core type and subscription format mismatch".to_string(),
            ));
        }
    };
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
/// Only local overrides (no remote URLs); for remote override overlay scenarios please use
/// [`build_core_config_v2`]. inbounds and MITM chain are not handled in this layer, injected
/// by `state` calling `compose_*`.
pub async fn build_core_config(
    core_type: CoreType,
    sub_content: &SubContent,
    overrides: &ProfileOverrides,
) -> PanelResult<Value> {
    build_core_config_v2(
        core_type,
        sub_content,
        &EffectiveOverrides {
            remote_yaml: String::new(),
            local_yaml: overrides.yaml_override.clone(),
            remote_js: String::new(),
            local_js: overrides.js_override.clone(),
        },
    )
    .await
}

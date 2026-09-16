//! Apply config slices to a sing-box config object (ADR-0005 §3.2).
//!
//! Rendering is split into pure functions ([`super::render_dns`] /
//! [`render_outbound`]) so it can be unit tested without a pipeline.
//! [`apply_config_slices`] only mutates the passed-in JSON value.
//!
//! Injection is content-driven (no slice master switches):
//! - DNS: injected when `dns.mode == takeover` (cross-platform unified).
//! - Outbounds: enabled items are appended (empty list = no-op).
//! - Experimental: merged when `cache_file.enabled`.
//! - Route: `final` / `default_domain_resolver` written when non-empty.

use std::collections::{HashMap, HashSet};

use pp_common::{PanelError, PanelResult};
use serde_json::{Map, Value};

use super::experimental::CacheFileSlice;
use super::outbound::{
    CustomOutbound, Hysteria2Outbound, OutboundProtocol, OutboundTls, OutboundTransport,
    SelectorOutbound, ShadowsocksOutbound, TrojanOutbound, UrlTestOutbound, VlessOutbound,
    VmessOutbound,
};
use super::{ConfigSlices, DnsMode, outbound_tag, render_dns, render_domain_resolver, str_value};

/// Result of [`apply_config_slices`], suitable for logging.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Whether the DNS slice replaced `config.dns`.
    pub dns_applied: bool,
    /// Final tags of the injected custom outbounds (in injection order).
    pub outbound_tags: Vec<String>,
    /// Tags renamed to avoid collisions with existing outbounds.
    pub renamed_outbounds: Vec<OutboundTagRename>,
}

/// A `slice-` tag that was renamed because it collided with an existing tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundTagRename {
    /// The requested (base) tag.
    pub from: String,
    /// The tag actually injected (base + stable numeric suffix).
    pub to: String,
}

/// Apply the enabled slices to `config`.
///
/// Injection is content-driven, with no slice master switches:
/// - DNS slice: replaces `config.dns` with the rendered DNS object when
///   `dns.mode == takeover`. [`DnsMode::FollowSystem`] (the default) keeps the
///   built-in / template DNS untouched. Same semantics on desktop and Android.
/// - Outbounds slice: appends rendered custom outbounds to `config.outbounds`,
///   renaming tags that collide with existing outbounds. Disabled items are
///   skipped; an empty item list is a no-op.
/// - Experimental slice: deep merges the rendered `cache_file` object into
///   `config.experimental`, preserving sibling keys (`clash_api`, …), when
///   `cache_file.enabled`.
/// - Route slice: deep merges `final` / `default_domain_resolver` into
///   `config.route`, preserving sibling keys (`rules`, `auto_detect_interface`,
///   …), for non-empty fields only.
///
/// Returns an error when `config` is not a JSON object (callers always pass a
/// config object).
pub fn apply_config_slices(config: &mut Value, slices: &ConfigSlices) -> PanelResult<ApplyReport> {
    let Some(obj) = config.as_object_mut() else {
        return Err(PanelError::Client(
            "apply_config_slices: config is not a JSON object".to_string(),
        ));
    };

    let mut report = ApplyReport::default();

    // DNS: content-driven. `FollowSystem` (default) keeps the built-in/template
    // DNS; only `Takeover` injects the slice body. Cross-platform unified.
    if slices.dns.mode == DnsMode::Takeover {
        obj.insert("dns".to_string(), render_dns(&slices.dns));
        report.dns_applied = true;
    }

    // 内置分组条目（2026-09 物化模型）：不追加新出站，而是把可调字段覆写到模板的同名
    // 内置分组上（成员列表保持模板动态计算）。先覆写，再处理用户自定义出站。
    let builtin_groups: Vec<&CustomOutbound> = slices
        .outbounds
        .items
        .iter()
        .filter(|item| item.builtin && item.enabled)
        .collect();
    for item in builtin_groups {
        super::apply_groups::apply_builtin_group_override(obj, item, &mut report);
    }

    let custom_items: Vec<&CustomOutbound> = slices
        .outbounds
        .items
        .iter()
        .filter(|item| !item.builtin)
        .collect();
    if !custom_items.is_empty() {
        let mut used = collect_outbound_tags(obj);
        let mut rendered = Vec::new();
        // Base tag -> final (possibly renamed) tag, used to remap group members
        // that reference a slice node outbound whose tag was renamed.
        let mut node_tag_map: HashMap<String, String> = HashMap::new();

        // Render concrete node outbounds first so group outbounds (rendered
        // below) come after the members they reference.
        for item in custom_items
            .iter()
            .filter(|item| item.enabled && !item.protocol.is_group())
        {
            let base = outbound_tag(&item.name);
            let (tag, renamed) = unique_tag(&base, &used);
            used.insert(tag.clone());
            if renamed {
                report.renamed_outbounds.push(OutboundTagRename {
                    from: base.clone(),
                    to: tag.clone(),
                });
            }
            node_tag_map.insert(base, tag.clone());
            report.outbound_tags.push(tag.clone());
            rendered.push(render_outbound(item, &tag));
        }

        // Then render selector / urltest groups.
        for item in custom_items
            .iter()
            .filter(|item| item.enabled && item.protocol.is_group())
        {
            let base = outbound_tag(&item.name);
            let (tag, renamed) = unique_tag(&base, &used);
            used.insert(tag.clone());
            if renamed {
                report.renamed_outbounds.push(OutboundTagRename {
                    from: base,
                    to: tag.clone(),
                });
            }
            report.outbound_tags.push(tag.clone());
            rendered.push(render_outbound_with(item, &tag, &node_tag_map));
        }
        append_outbounds(obj, rendered);
    }

    if slices.experimental.cache_file.enabled {
        // Deep merge: only the `cache_file` key is written, sibling keys such as
        // `clash_api` (owned by the ④ panel-feature layer) are preserved.
        let experimental = obj
            .entry("experimental".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(exp) = experimental.as_object_mut() {
            exp.insert(
                "cache_file".to_string(),
                render_cache_file(&slices.experimental.cache_file),
            );
        }
    }

    if !slices.route.final_tag.is_empty() || !slices.route.resolver.server.is_empty() {
        // Deep merge: only `final` / `default_domain_resolver` are written, so
        // sibling keys (`rules`, `auto_detect_interface`, …) are preserved.
        // `default_domain_resolver` is written as the 1.12+ object form; the ④
        // `compose_singbox_config` layer only fills it when absent, so a value
        // set here survives composition.
        let route = obj
            .entry("route".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(route) = route.as_object_mut() {
            if !slices.route.final_tag.is_empty() {
                route.insert("final".to_string(), str_value(&slices.route.final_tag));
            }
            if !slices.route.resolver.server.is_empty() {
                route.insert(
                    "default_domain_resolver".to_string(),
                    render_domain_resolver(&slices.route.resolver),
                );
            }
        }
    }

    Ok(report)
}

/// Render a [`CacheFileSlice`] as a sing-box `experimental.cache_file` object.
///
/// `enabled` is always emitted (explicit `false` disables the cache file).
/// Optional fields are only emitted when the cache file is enabled and the
/// value is set: empty `path` / `cache_id` fall back to sing-box defaults, and
/// `store_fakeip = false` is the sing-box default.
#[must_use]
pub fn render_cache_file(cache_file: &CacheFileSlice) -> Value {
    let mut out = Map::new();
    out.insert("enabled".to_string(), Value::Bool(cache_file.enabled));
    if cache_file.enabled {
        if !cache_file.path.is_empty() {
            out.insert("path".to_string(), str_value(&cache_file.path));
        }
        if !cache_file.cache_id.is_empty() {
            out.insert("cache_id".to_string(), str_value(&cache_file.cache_id));
        }
        if cache_file.store_fakeip {
            out.insert("store_fakeip".to_string(), Value::Bool(true));
        }
    }
    Value::Object(out)
}

/// Render a [`CustomOutbound`] as a sing-box outbound object with `tag`.
#[must_use]
pub fn render_outbound(item: &CustomOutbound, tag: &str) -> Value {
    render_outbound_with(item, tag, &HashMap::new())
}

/// Render a [`CustomOutbound`], remapping group member tags through
/// `node_tag_map` (base tag -> final tag after collision renames).
fn render_outbound_with(
    item: &CustomOutbound,
    tag: &str,
    node_tag_map: &HashMap<String, String>,
) -> Value {
    let mut out = Map::new();
    out.insert("tag".to_string(), str_value(tag));
    match &item.protocol {
        OutboundProtocol::Vless(o) => render_vless(&mut out, o),
        OutboundProtocol::Vmess(o) => render_vmess(&mut out, o),
        OutboundProtocol::Shadowsocks(o) => render_shadowsocks(&mut out, o),
        OutboundProtocol::Trojan(o) => render_trojan(&mut out, o),
        OutboundProtocol::Hysteria2(o) => render_hysteria2(&mut out, o),
        OutboundProtocol::Selector(o) => render_selector(&mut out, o, node_tag_map),
        OutboundProtocol::UrlTest(o) => render_urltest(&mut out, o, node_tag_map),
    }
    Value::Object(out)
}

fn render_selector(
    out: &mut Map<String, Value>,
    o: &SelectorOutbound,
    node_tag_map: &HashMap<String, String>,
) {
    out.insert("type".to_string(), str_value("selector"));
    out.insert(
        "outbounds".to_string(),
        render_members(&o.outbounds, node_tag_map),
    );
    if !o.default.is_empty() {
        out.insert(
            "default".to_string(),
            str_value(remap_tag(&o.default, node_tag_map)),
        );
    }
    if o.interrupt_exist_connections {
        out.insert("interrupt_exist_connections".to_string(), Value::Bool(true));
    }
}

fn render_urltest(
    out: &mut Map<String, Value>,
    o: &UrlTestOutbound,
    node_tag_map: &HashMap<String, String>,
) {
    out.insert("type".to_string(), str_value("urltest"));
    out.insert(
        "outbounds".to_string(),
        render_members(&o.outbounds, node_tag_map),
    );
    if !o.url.is_empty() {
        out.insert("url".to_string(), str_value(&o.url));
    }
    if !o.interval.is_empty() {
        out.insert("interval".to_string(), str_value(&o.interval));
    }
    if o.tolerance != 0 {
        out.insert("tolerance".to_string(), Value::from(o.tolerance));
    }
    if o.interrupt_exist_connections {
        out.insert("interrupt_exist_connections".to_string(), Value::Bool(true));
    }
}

/// Render a group member list as a JSON array, remapping renamed node tags.
fn render_members(members: &[String], node_tag_map: &HashMap<String, String>) -> Value {
    Value::Array(
        members
            .iter()
            .map(|member| str_value(remap_tag(member, node_tag_map)))
            .collect(),
    )
}

/// Resolve a member tag to its final tag, if it was renamed.
fn remap_tag<'a>(tag: &'a str, node_tag_map: &'a HashMap<String, String>) -> &'a str {
    node_tag_map.get(tag).map_or(tag, String::as_str)
}

fn render_vless(out: &mut Map<String, Value>, o: &VlessOutbound) {
    out.insert("type".to_string(), str_value("vless"));
    insert_server(out, &o.server, o.server_port);
    out.insert("uuid".to_string(), str_value(&o.uuid));
    if !o.flow.is_empty() {
        out.insert("flow".to_string(), str_value(&o.flow));
    }
    insert_tls(out, &o.tls);
    insert_transport(out, &o.transport);
}

fn render_vmess(out: &mut Map<String, Value>, o: &VmessOutbound) {
    out.insert("type".to_string(), str_value("vmess"));
    insert_server(out, &o.server, o.server_port);
    out.insert("uuid".to_string(), str_value(&o.uuid));
    if !o.security.is_empty() {
        out.insert("security".to_string(), str_value(&o.security));
    }
    if o.alter_id != 0 {
        out.insert("alter_id".to_string(), Value::from(o.alter_id));
    }
    insert_tls(out, &o.tls);
    insert_transport(out, &o.transport);
}

fn render_shadowsocks(out: &mut Map<String, Value>, o: &ShadowsocksOutbound) {
    out.insert("type".to_string(), str_value("shadowsocks"));
    insert_server(out, &o.server, o.server_port);
    out.insert("method".to_string(), str_value(&o.method));
    out.insert("password".to_string(), str_value(&o.password));
}

fn render_trojan(out: &mut Map<String, Value>, o: &TrojanOutbound) {
    out.insert("type".to_string(), str_value("trojan"));
    insert_server(out, &o.server, o.server_port);
    out.insert("password".to_string(), str_value(&o.password));
    insert_tls(out, &o.tls);
    insert_transport(out, &o.transport);
}

fn render_hysteria2(out: &mut Map<String, Value>, o: &Hysteria2Outbound) {
    out.insert("type".to_string(), str_value("hysteria2"));
    insert_server(out, &o.server, o.server_port);
    out.insert("password".to_string(), str_value(&o.password));
    if o.up_mbps != 0 {
        out.insert("up_mbps".to_string(), Value::from(o.up_mbps));
    }
    if o.down_mbps != 0 {
        out.insert("down_mbps".to_string(), Value::from(o.down_mbps));
    }
    if !o.obfs.obfs_type.is_empty() {
        let mut obfs = Map::new();
        obfs.insert("type".to_string(), str_value(&o.obfs.obfs_type));
        if !o.obfs.password.is_empty() {
            obfs.insert("password".to_string(), str_value(&o.obfs.password));
        }
        out.insert("obfs".to_string(), Value::Object(obfs));
    }
    insert_tls(out, &o.tls);
}

fn insert_server(out: &mut Map<String, Value>, server: &str, port: u16) {
    out.insert("server".to_string(), str_value(server));
    out.insert("server_port".to_string(), Value::from(port));
}

fn insert_tls(out: &mut Map<String, Value>, tls: &OutboundTls) {
    if !tls.enabled {
        return;
    }
    let mut value = Map::new();
    value.insert("enabled".to_string(), Value::Bool(true));
    if !tls.server_name.is_empty() {
        value.insert("server_name".to_string(), str_value(&tls.server_name));
    }
    if tls.insecure {
        value.insert("insecure".to_string(), Value::Bool(true));
    }
    if !tls.alpn.is_empty() {
        value.insert(
            "alpn".to_string(),
            Value::Array(tls.alpn.iter().map(|s| str_value(s)).collect()),
        );
    }
    out.insert("tls".to_string(), Value::Object(value));
}

fn insert_transport(out: &mut Map<String, Value>, transport: &OutboundTransport) {
    let kind = transport.kind.as_str();
    if kind.is_empty() || kind == "tcp" {
        return;
    }
    let mut value = Map::new();
    value.insert("type".to_string(), str_value(kind));
    match kind {
        "ws" => {
            if !transport.path.is_empty() {
                value.insert("path".to_string(), str_value(&transport.path));
            }
            if !transport.host.is_empty() {
                let mut headers = Map::new();
                headers.insert("Host".to_string(), str_value(&transport.host));
                value.insert("headers".to_string(), Value::Object(headers));
            }
        }
        "grpc" => {
            if !transport.path.is_empty() {
                value.insert("service_name".to_string(), str_value(&transport.path));
            }
        }
        "http" => {
            if !transport.path.is_empty() {
                value.insert("path".to_string(), str_value(&transport.path));
            }
            if !transport.host.is_empty() {
                value.insert(
                    "host".to_string(),
                    Value::Array(vec![str_value(&transport.host)]),
                );
            }
        }
        "httpupgrade" => {
            if !transport.path.is_empty() {
                value.insert("path".to_string(), str_value(&transport.path));
            }
            if !transport.host.is_empty() {
                value.insert("host".to_string(), str_value(&transport.host));
            }
        }
        _ => {
            if !transport.path.is_empty() {
                value.insert("path".to_string(), str_value(&transport.path));
            }
        }
    }
    out.insert("transport".to_string(), Value::Object(value));
}

/// Collect all existing outbound tags from a config object.
fn collect_outbound_tags(obj: &Map<String, Value>) -> HashSet<String> {
    obj.get("outbounds")
        .and_then(Value::as_array)
        .map(|outbounds| {
            outbounds
                .iter()
                .filter_map(|outbound| outbound.get("tag").and_then(Value::as_str))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Append rendered outbounds to `config.outbounds`, creating the array if needed.
fn append_outbounds(obj: &mut Map<String, Value>, rendered: Vec<Value>) {
    if rendered.is_empty() {
        return;
    }
    let outbounds = obj
        .entry("outbounds".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(array) = outbounds.as_array_mut() {
        array.extend(rendered);
    } else {
        *outbounds = Value::Array(rendered);
    }
}

/// Resolve a collision-free tag: `base`, then `base-2`, `base-3`, … Returns the
/// tag and whether it was renamed.
fn unique_tag(base: &str, used: &HashSet<String>) -> (String, bool) {
    if !used.contains(base) {
        return (base.to_string(), false);
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{base}-{n}");
        if !used.contains(&candidate) {
            return (candidate, true);
        }
        n += 1;
    }
}

#[cfg(test)]
#[path = "tests/apply_tests.rs"]
mod tests;

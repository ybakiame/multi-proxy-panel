//! Apply config slices to a sing-box config object (ADR-0005 §3.2).
//!
//! Rendering is split into pure functions ([`render_dns`] / [`render_outbound`])
//! so it can be unit tested without a pipeline. [`apply_config_slices`] only
//! mutates the passed-in JSON value.

use std::collections::HashSet;

use pp_common::{PanelError, PanelResult};
use serde_json::{Map, Value};

use super::dns::{DnsRule, DnsServer};
use super::outbound::{
    CustomOutbound, Hysteria2Outbound, OutboundProtocol, OutboundTls, OutboundTransport,
    ShadowsocksOutbound, TrojanOutbound, VlessOutbound, VmessOutbound,
};
use super::{ConfigSlices, DnsSlice, outbound_tag};

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
/// - DNS slice: replaces `config.dns` with the rendered DNS object.
/// - Outbounds slice: appends rendered custom outbounds to `config.outbounds`,
///   renaming tags that collide with existing outbounds.
///
/// A disabled slice leaves the config untouched. Returns an error when `config`
/// is not a JSON object (callers always pass a config object).
pub fn apply_config_slices(config: &mut Value, slices: &ConfigSlices) -> PanelResult<ApplyReport> {
    let Some(obj) = config.as_object_mut() else {
        return Err(PanelError::Client(
            "apply_config_slices: config is not a JSON object".to_string(),
        ));
    };

    let mut report = ApplyReport::default();

    if slices.dns.enabled {
        obj.insert("dns".to_string(), render_dns(&slices.dns));
        report.dns_applied = true;
    }

    if slices.outbounds.enabled {
        let mut used = collect_outbound_tags(obj);
        let mut rendered = Vec::new();
        for item in slices.outbounds.items.iter().filter(|item| item.enabled) {
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
            rendered.push(render_outbound(item, &tag));
        }
        append_outbounds(obj, rendered);
    }

    Ok(report)
}

/// Render a [`DnsSlice`] as a sing-box 1.12+ type-based DNS object.
///
/// Every server entry carries `type`; [`super::DnsServerType::Local`] entries
/// omit `server` / `server_port`. The top-level `strategy` is always emitted.
#[must_use]
pub fn render_dns(dns: &DnsSlice) -> Value {
    let servers: Vec<Value> = dns.servers.iter().map(render_dns_server).collect();
    let rules: Vec<Value> = dns
        .rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(render_dns_rule)
        .collect();

    let mut out = Map::new();
    out.insert("servers".to_string(), Value::Array(servers));
    out.insert("rules".to_string(), Value::Array(rules));
    if !dns.final_tag.is_empty() {
        out.insert("final".to_string(), str_value(&dns.final_tag));
    }
    out.insert("strategy".to_string(), str_value(dns.strategy.as_str()));
    Value::Object(out)
}

/// Render a single DNS server entry (sing-box 1.12+ new format).
fn render_dns_server(server: &DnsServer) -> Value {
    let mut out = Map::new();
    out.insert("tag".to_string(), str_value(&server.tag));
    out.insert("type".to_string(), str_value(server.server_type.as_str()));
    if server.server_type.uses_server() {
        out.insert("server".to_string(), str_value(&server.server));
        if let Some(port) = server.server_port {
            out.insert("server_port".to_string(), Value::from(port));
        }
    }
    if !server.detour.is_empty() {
        out.insert("detour".to_string(), str_value(&server.detour));
    }
    if !server.domain_resolver.is_empty() {
        out.insert(
            "domain_resolver".to_string(),
            str_value(&server.domain_resolver),
        );
    }
    Value::Object(out)
}

/// Render a single DNS rule (`action: "route"` + `server` tag).
fn render_dns_rule(rule: &DnsRule) -> Value {
    let mut out = Map::new();
    out.insert(
        rule.match_type.field().to_string(),
        Value::Array(vec![str_value(&rule.target)]),
    );
    out.insert("action".to_string(), str_value("route"));
    out.insert("server".to_string(), str_value(&rule.server_tag));
    Value::Object(out)
}

/// Render a [`CustomOutbound`] as a sing-box outbound object with `tag`.
#[must_use]
pub fn render_outbound(item: &CustomOutbound, tag: &str) -> Value {
    let mut out = Map::new();
    out.insert("tag".to_string(), str_value(tag));
    match &item.protocol {
        OutboundProtocol::Vless(o) => render_vless(&mut out, o),
        OutboundProtocol::Vmess(o) => render_vmess(&mut out, o),
        OutboundProtocol::Shadowsocks(o) => render_shadowsocks(&mut out, o),
        OutboundProtocol::Trojan(o) => render_trojan(&mut out, o),
        OutboundProtocol::Hysteria2(o) => render_hysteria2(&mut out, o),
    }
    Value::Object(out)
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

/// Short helper for a JSON string value.
fn str_value(value: &str) -> Value {
    Value::String(value.to_string())
}

#[cfg(test)]
#[path = "tests/apply_tests.rs"]
mod tests;

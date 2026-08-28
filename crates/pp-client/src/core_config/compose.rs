//! Subscription config composition (sing-box and mihomo).

use pp_common::{PanelError, PanelResult};
use serde_json::{Value, json};

use super::MitmChain;

/// Compose subscription config into a locally usable sing-box config.
///
/// Without MITM chain, inbounds are replaced wholesale with a single mixed inbound
/// (only listening on `127.0.0.1`), and `outbounds` / `route` / `dns` / `log` etc. are preserved.
///
/// With MITM chain (`mitm_chain = Some`):
/// - inbounds are two mixed: main inlet `main-in` (`mixed_port`) + return inlet
///   `mitm-return` (`return_port`, MITM-decrypted traffic returns to core normal routing from here)
/// - outbounds append `pp-mitm` http outbound (pointing to MITM listen address)
/// - route.rules prepend a whitelist rule: `inbound = [main-in]`, `*.` prefix hostnames go to
///   `domain_suffix`, rest go to `domain`, `outbound = "pp-mitm"`
///
/// Both paths perform sing-box 1.12+ DNS compatibility adaptation: when subscription contains
/// `dns.servers`, ensure `route.default_domain_resolver` exists (pointing to first DNS server tag,
/// auto-generated when missing), otherwise current sing-box rejects config.
pub fn compose_singbox_config(
    sub_config: &Value,
    mixed_port: u16,
    mitm_chain: Option<MitmChain>,
) -> PanelResult<Value> {
    let mut obj = sub_config.as_object().cloned().ok_or_else(|| {
        PanelError::Client("subscription config must be a JSON object".to_string())
    })?;

    ensure_domain_resolver(&mut obj);

    let Some(chain) = mitm_chain else {
        let mixed_inbound = json!({
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": mixed_port,
        });
        obj.insert("inbounds".to_string(), Value::Array(vec![mixed_inbound]));
        return Ok(Value::Object(obj));
    };

    let main_in = json!({
        "type": "mixed",
        "tag": "main-in",
        "listen": "127.0.0.1",
        "listen_port": mixed_port,
    });
    let return_in = json!({
        "type": "mixed",
        "tag": "mitm-return",
        "listen": "127.0.0.1",
        "listen_port": chain.return_port,
    });
    obj.insert(
        "inbounds".to_string(),
        Value::Array(vec![main_in, return_in]),
    );

    // outbounds append pp-mitm http outbound.
    let mut outbounds = obj
        .remove("outbounds")
        .and_then(|o| o.as_array().cloned())
        .unwrap_or_default();
    outbounds.push(json!({
        "type": "http",
        "tag": "pp-mitm",
        "server": "127.0.0.1",
        "server_port": chain.proxy_addr.port(),
    }));
    obj.insert("outbounds".to_string(), Value::Array(outbounds));

    // route rules prepend whitelist rule (create route when missing).
    let mut route = obj
        .remove("route")
        .and_then(|r| r.as_object().cloned())
        .unwrap_or_default();
    let mut domain = Vec::new();
    let mut domain_suffix = Vec::new();
    for hostname in &chain.hostnames {
        // `-` / `!` prefix are exclusions (not intercepted), not generated into core routing rules:
        // corresponding domain traffic goes direct, not sent to MITM inbound.
        if hostname.starts_with('-') || hostname.starts_with('!') {
            continue;
        }
        match hostname.strip_prefix("*.") {
            Some(suffix) => domain_suffix.push(Value::String(suffix.to_string())),
            None => domain.push(Value::String(hostname.clone())),
        }
    }
    let mut rule = serde_json::Map::new();
    rule.insert("inbound".to_string(), json!(["main-in"]));
    if !domain.is_empty() {
        rule.insert("domain".to_string(), Value::Array(domain));
    }
    if !domain_suffix.is_empty() {
        rule.insert("domain_suffix".to_string(), Value::Array(domain_suffix));
    }
    rule.insert("outbound".to_string(), Value::String("pp-mitm".to_string()));
    let mut rules = route
        .remove("rules")
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();
    rules.insert(0, Value::Object(rule));
    route.insert("rules".to_string(), Value::Array(rules));
    obj.insert("route".to_string(), Value::Object(route));
    Ok(Value::Object(obj))
}

/// sing-box 1.12+ DNS compatibility adaptation.
///
/// sing-box 1.12+ requires explicit domain resolver declaration: when config contains
/// `dns.servers` (and non-empty), `route.default_domain_resolver` must exist, otherwise
/// config check directly rejects (`missing route.default_domain_resolver`). Subscription templates
/// usually only declare `dns.servers`, this function takes tag from first server (auto-generates
/// `dns-<i>` when missing) and writes to `route.default_domain_resolver`.
pub(crate) fn ensure_domain_resolver(obj: &mut serde_json::Map<String, Value>) {
    let has_servers = obj
        .get("dns")
        .and_then(|d| d.as_object())
        .and_then(|d| d.get("servers"))
        .and_then(|s| s.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);
    if !has_servers {
        return;
    }
    // Skip when explicit resolver already exists.
    let has_resolver = obj
        .get("route")
        .and_then(|r| r.as_object())
        .and_then(|r| r.get("default_domain_resolver"))
        .is_some();
    if has_resolver {
        return;
    }

    let mut resolver_tag: Option<String> = None;
    if let Some(servers) = obj
        .get_mut("dns")
        .and_then(|d| d.as_object_mut())
        .and_then(|d| d.get_mut("servers"))
        .and_then(|s| s.as_array_mut())
    {
        for (i, server) in servers.iter_mut().enumerate() {
            if let Some(s) = server.as_object_mut() {
                if !s.contains_key("tag") {
                    s.insert("tag".to_string(), Value::String(format!("dns-{i}")));
                }
                if let Some(tag) = s.get("tag").and_then(|t| t.as_str()) {
                    resolver_tag = Some(tag.to_string());
                    break;
                }
            }
        }
    }

    let Some(tag) = resolver_tag else { return };
    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(r) = route.as_object_mut() {
        r.insert(
            "default_domain_resolver".to_string(),
            json!({ "server": tag }),
        );
    }
}

/// Compose clash YAML subscription config into mihomo local startup config.
///
/// Parse YAML and inject local mixed inlet: without MITM chain, write `mixed-port`, remove
/// conflicting `port` / `socks-port` / `redir-port` / `tproxy-port` (client only keeps mixed
/// inbound), and use `allow-lan: false`, `bind-address: 127.0.0.1` as security defaults.
/// `proxies` / `proxy-groups` / `rules` / `dns` etc. are preserved.
///
/// With MITM chain (`mitm_chain = Some`), use explicit `listeners` to declare main inlet
/// `main-in` (`mixed-port` is replaced, ensuring `IN-NAME` can match) and return inlet
/// `mitm-return`, `proxies` append `pp-mitm` http proxy, `rules` prepend whitelist rules
/// (`AND,((IN-NAME,main-in),(DOMAIN-SUFFIX,<suffix>)),pp-mitm` /
/// `AND,((IN-NAME,main-in),(DOMAIN,<exact>)),pp-mitm`).
pub fn compose_mihomo_config(
    sub_yaml: &str,
    mixed_port: u16,
    mitm_chain: Option<MitmChain>,
) -> PanelResult<Value> {
    let value: Value = serde_yaml::from_str(sub_yaml)
        .map_err(|e| PanelError::Client(format!("invalid clash config in subscription: {e}")))?;
    let mut obj = value
        .as_object()
        .cloned()
        .ok_or_else(|| PanelError::Client("clash config must be a YAML mapping".to_string()))?;

    for key in ["port", "socks-port", "redir-port", "tproxy-port"] {
        obj.remove(key);
    }
    obj.insert("allow-lan".to_string(), Value::Bool(false));
    obj.insert(
        "bind-address".to_string(),
        Value::String("127.0.0.1".to_string()),
    );

    let Some(chain) = mitm_chain else {
        obj.insert("mixed-port".to_string(), Value::from(mixed_port));
        return Ok(Value::Object(obj));
    };

    // Explicit listeners declare main inlet and return inlet, replacing top-level mixed-port,
    // ensuring IN-NAME can match.
    obj.remove("mixed-port");
    obj.insert(
        "listeners".to_string(),
        Value::Array(vec![
            json!({
                "name": "main-in",
                "type": "mixed",
                "port": mixed_port,
                "listen": "127.0.0.1",
            }),
            json!({
                "name": "mitm-return",
                "type": "mixed",
                "port": chain.return_port,
                "listen": "127.0.0.1",
            }),
        ]),
    );

    // proxies append pp-mitm http proxy.
    let mut proxies = obj
        .remove("proxies")
        .and_then(|p| p.as_array().cloned())
        .unwrap_or_default();
    proxies.push(json!({
        "name": "pp-mitm",
        "type": "http",
        "server": "127.0.0.1",
        "port": chain.proxy_addr.port(),
    }));
    obj.insert("proxies".to_string(), Value::Array(proxies));

    // rules prepend whitelist rules (one per domain, original rules preserved).
    // Logic rule syntax is `LOGIC_TYPE,((payload1),(payload2)),Proxy`: comma required between
    // AND and `((` (mihomo parses rule name and parameters by `,`).
    let rules = obj
        .remove("rules")
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();
    let mut prepended = Vec::new();
    for hostname in &chain.hostnames {
        // `-` / `!` prefix are exclusions (not intercepted), no core routing rules generated:
        // corresponding domain traffic goes direct, not sent to MITM inbound.
        if hostname.starts_with('-') || hostname.starts_with('!') {
            continue;
        }
        let rule = match hostname.strip_prefix("*.") {
            Some(suffix) => {
                format!("AND,((IN-NAME,main-in),(DOMAIN-SUFFIX,{suffix})),pp-mitm")
            }
            None => format!("AND,((IN-NAME,main-in),(DOMAIN,{hostname})),pp-mitm"),
        };
        prepended.push(Value::String(rule));
    }
    prepended.extend(rules);
    obj.insert("rules".to_string(), Value::Array(prepended));
    Ok(Value::Object(obj))
}

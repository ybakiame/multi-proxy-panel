//! 核心配置合成：将订阅配置合成为本地可用的核心启动配置。

use std::net::SocketAddr;

use pp_common::{PanelError, PanelResult};
use serde_json::{Value, json};

/// MITM 链路信息：核心配置合成时注入 MITM 路由所需的入站 / 出站 / 规则。
pub struct MitmChain {
    /// MITM 代理监听地址（核心路由规则指向的 http outbound 目标）。
    pub proxy_addr: SocketAddr,
    /// MITM 回流入口端口（核心回流 mixed 入站监听端口，通常为 `mixed_port + 1`）。
    pub return_port: u16,
    /// 需经 MITM 的白名单主机名（`*.` 前缀按后缀匹配，其余精确匹配）。
    pub hostnames: Vec<String>,
}

/// 将订阅配置合成为可直接传给 sing-box 的本地配置。
///
/// 不带 MITM 链路时，入站整体替换为单个 mixed 入站（仅监听 `127.0.0.1`），
/// `outbounds` / `route` / `dns` / `log` 等其余字段原样保留。
///
/// 带 MITM 链路时（`mitm_chain = Some`）：
/// - inbounds 为两个 mixed：主入口 `main-in`（`mixed_port`）+ 回流入口
///   `mitm-return`（`return_port`，MITM 解密后的流量从此回流核心正常路由）
/// - outbounds 追加 `pp-mitm` http outbound（指向 MITM 监听地址）
/// - route.rules 前插一条白名单规则：`inbound = [main-in]`，`*.` 前缀主机名
///   进入 `domain_suffix`、其余进入 `domain`，`outbound = "pp-mitm"`
///
/// 两路径均会做 sing-box 1.12+ 的 DNS 兼容适配：订阅含 `dns.servers` 时，
/// 保证 `route.default_domain_resolver` 存在（指向第一个 DNS server 的 tag，
/// 缺 tag 时自动生成），否则配置被当前 sing-box 拒绝。
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

    // outbounds 追加 pp-mitm http outbound。
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

    // route 规则前插白名单规则（route 缺失时创建）。
    let mut route = obj
        .remove("route")
        .and_then(|r| r.as_object().cloned())
        .unwrap_or_default();
    let mut domain = Vec::new();
    let mut domain_suffix = Vec::new();
    for hostname in &chain.hostnames {
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

/// sing-box 1.12+ DNS 兼容适配。
///
/// sing-box 1.12 起要求 DNS 相关配置显式声明 domain resolver：配置含
/// `dns.servers`（且非空）时，`route.default_domain_resolver` 必须存在，否则
/// 配置检查直接拒绝（`missing route.default_domain_resolver`）。订阅模板通常
/// 只声明 `dns.servers`，本函数为第一个 server 取 tag（缺 tag 时按索引生成
/// `dns-<i>`）并写入 `route.default_domain_resolver`。
fn ensure_domain_resolver(obj: &mut serde_json::Map<String, Value>) {
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
    // 已有显式 resolver 则跳过。
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

/// 将 clash YAML 订阅配置合成为 mihomo 本地启动配置。
///
/// 解析 YAML 后注入本地 mixed 入口：不带 MITM 链路时写入 `mixed-port`，移除
/// 冲突的 `port` / `socks-port` / `redir-port` / `tproxy-port`（客户端只保留
/// mixed 入站），并以 `allow-lan: false`、`bind-address: 127.0.0.1` 作为安全
/// 默认。`proxies` / `proxy-groups` / `rules` / `dns` 等其余字段原样保留。
///
/// 带 MITM 链路时（`mitm_chain = Some`）改用显式 `listeners` 声明主入口
/// `main-in`（`mixed-port` 被替代，确保 `IN-NAME` 可匹配）与回流入口
/// `mitm-return`，`proxies` 追加 `pp-mitm` http 代理，`rules` 前插白名单规则
/// （`AND,((IN-NAME,main-in),(DOMAIN-SUFFIX,<suffix>)),pp-mitm` /
/// `AND,((IN-NAME,main-in),(DOMAIN,<exact>)),pp-mitm`）。
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

    // 显式 listeners 声明主入口与回流入口，替代顶层 mixed-port，确保 IN-NAME 可匹配。
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

    // proxies 追加 pp-mitm http 代理。
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

    // rules 前插白名单规则（每域名一条，原规则保留）。
    // 逻辑规则语法为 `LOGIC_TYPE,((payload1),(payload2)),Proxy`：`AND` 与
    // `((` 之间必须有逗号（mihomo 按 `,` 解析规则名与参数）。
    let rules = obj
        .remove("rules")
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();
    let mut prepended = Vec::new();
    for hostname in &chain.hostnames {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_subscription() -> Value {
        json!({
            "inbounds": [{ "type": "vless", "tag": "hub-in", "listen_port": 443 }],
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "n1" },
            "dns": { "servers": ["1.1.1.1"] },
            "log": { "level": "info" }
        })
    }

    #[test]
    fn compose_singbox_replaces_inbounds_and_preserves_rest() {
        let sub = sample_subscription();
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();

        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 1);
        assert_eq!(inbounds[0]["type"], "mixed");
        assert_eq!(inbounds[0]["tag"], "mixed-in");
        assert_eq!(inbounds[0]["listen"], "127.0.0.1");
        assert_eq!(inbounds[0]["listen_port"], 17890);

        assert_eq!(cfg["outbounds"], sub["outbounds"]);
        assert_eq!(cfg["route"], sub["route"]);
        assert_eq!(cfg["dns"], sub["dns"]);
        assert_eq!(cfg["log"], sub["log"]);
    }

    #[test]
    fn compose_singbox_rejects_non_object() {
        let err = compose_singbox_config(&json!([1, 2, 3]), 17890, None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn compose_singbox_with_mitm_chain_injects_inbounds_outbound_and_route() {
        let sub = json!({
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 }
            ],
            "route": { "final": "n1" },
        });
        let chain = MitmChain {
            proxy_addr: "127.0.0.1:34567".parse().unwrap(),
            return_port: 17891,
            hostnames: vec!["example.com".to_string(), "*.cdn.example.net".to_string()],
        };
        let cfg = compose_singbox_config(&sub, 17890, Some(chain)).unwrap();

        // 双 mixed 入站：主入口 + 回流入口。
        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2);
        assert_eq!(inbounds[0]["type"], "mixed");
        assert_eq!(inbounds[0]["tag"], "main-in");
        assert_eq!(inbounds[0]["listen"], "127.0.0.1");
        assert_eq!(inbounds[0]["listen_port"], 17890);
        assert_eq!(inbounds[1]["tag"], "mitm-return");
        assert_eq!(inbounds[1]["listen_port"], 17891);

        // pp-mitm http outbound 追加在原有 outbound 之后。
        let outbounds = cfg["outbounds"].as_array().unwrap();
        assert_eq!(outbounds.len(), 2);
        assert_eq!(outbounds[1]["tag"], "pp-mitm");
        assert_eq!(outbounds[1]["type"], "http");
        assert_eq!(outbounds[1]["server"], "127.0.0.1");
        assert_eq!(outbounds[1]["server_port"], 34567);

        // route 规则前插：inbound 匹配主入口，精确/后缀正确分流，final 保留。
        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["inbound"], json!(["main-in"]));
        assert_eq!(rules[0]["domain"], json!(["example.com"]));
        assert_eq!(rules[0]["domain_suffix"], json!(["cdn.example.net"]));
        assert_eq!(rules[0]["outbound"], "pp-mitm");
        assert_eq!(cfg["route"]["final"], "n1");
    }

    #[test]
    fn compose_singbox_with_mitm_chain_creates_route_when_missing() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let chain = MitmChain {
            proxy_addr: "127.0.0.1:34567".parse().unwrap(),
            return_port: 17891,
            hostnames: vec!["example.com".to_string()],
        };
        let cfg = compose_singbox_config(&sub, 17890, Some(chain)).unwrap();

        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["inbound"], json!(["main-in"]));
        assert_eq!(rules[0]["domain"], json!(["example.com"]));
        assert_eq!(rules[0]["outbound"], "pp-mitm");
    }

    /// sing-box 1.12+ 要求 `route.default_domain_resolver` 指向已声明 tag 的
    /// DNS server，否则真实 sing-box 拒绝配置（legacy resolver 缺失）。
    #[test]
    fn compose_singbox_injects_default_domain_resolver_when_dns_present() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "dns": { "servers": [{ "type": "udp", "tag": "dns1", "server": "1.1.1.1" }] }
        });
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();

        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "dns1" })
        );
        // 已有 tag 的 server 不被改写。
        assert_eq!(cfg["dns"]["servers"][0]["tag"], "dns1");
    }

    #[test]
    fn compose_singbox_generates_tag_for_tagless_dns_server() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "dns": { "servers": [{ "type": "udp", "server": "1.1.1.1" }] }
        });
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();

        // 无 tag 的 server 自动补 tag，resolver 指向它。
        assert_eq!(cfg["dns"]["servers"][0]["tag"], "dns-0");
        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "dns-0" })
        );
    }

    #[test]
    fn compose_singbox_keeps_existing_resolver_and_skips_when_no_dns() {
        // 订阅已显式声明 resolver → 不覆盖。
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "dns": { "servers": [{ "type": "udp", "tag": "a", "server": "1.1.1.1" }] },
            "route": { "default_domain_resolver": { "server": "a" } }
        });
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "a" })
        );

        // 无 dns → 不注入 resolver。
        let bare = json!({ "outbounds": [{ "type": "direct", "tag": "direct" }] });
        let cfg = compose_singbox_config(&bare, 17890, None).unwrap();
        assert!(cfg["route"].get("default_domain_resolver").is_none());
    }

    #[test]
    fn compose_mihomo_injects_mixed_and_removes_conflicting_ports() {
        let yaml = r#"
port: 7890
socks-port: 7891
redir-port: 7892
tproxy-port: 7893
proxies:
  - name: n1
    type: vless
    server: example.com
    port: 443
proxy-groups:
  - name: PROXY
    type: select
    proxies: [n1]
rules:
  - DOMAIN-SUFFIX,example.com,PROXY
dns:
  enable: true
"#;
        let cfg = compose_mihomo_config(yaml, 17890, None).unwrap();

        // mixed 入口注入，冲突的独立端口字段移除。
        assert_eq!(cfg["mixed-port"], 17890);
        assert!(cfg.get("port").is_none());
        assert!(cfg.get("socks-port").is_none());
        assert!(cfg.get("redir-port").is_none());
        assert!(cfg.get("tproxy-port").is_none());

        // 安全默认。
        assert_eq!(cfg["allow-lan"], false);
        assert_eq!(cfg["bind-address"], "127.0.0.1");

        // 其余字段原样保留（含 proxies 条目内部的 server port）。
        assert_eq!(cfg["proxies"][0]["server"], "example.com");
        assert_eq!(cfg["proxies"][0]["port"], 443);
        assert_eq!(cfg["proxy-groups"][0]["name"], "PROXY");
        assert_eq!(cfg["rules"][0], "DOMAIN-SUFFIX,example.com,PROXY");
        assert_eq!(cfg["dns"]["enable"], true);
    }

    #[test]
    fn compose_mihomo_overrides_existing_mixed_port() {
        let yaml = "mixed-port: 7890\nallow-lan: true\nproxies:\n  - name: n1\n    type: direct\n";
        let cfg = compose_mihomo_config(yaml, 17890, None).unwrap();

        assert_eq!(cfg["mixed-port"], 17890);
        assert_eq!(cfg["allow-lan"], false);
        assert_eq!(cfg["bind-address"], "127.0.0.1");
    }

    #[test]
    fn compose_mihomo_with_mitm_chain_injects_listeners_proxy_and_rules() {
        let yaml = r#"
port: 7890
proxies:
  - name: n1
    type: vless
    server: example.com
    port: 443
rules:
  - MATCH,DIRECT
"#;
        let chain = MitmChain {
            proxy_addr: "127.0.0.1:34567".parse().unwrap(),
            return_port: 17891,
            hostnames: vec!["example.com".to_string(), "*.cdn.example.net".to_string()],
        };
        let cfg = compose_mihomo_config(yaml, 17890, Some(chain)).unwrap();

        // 主入口 + 回流入口走显式 listeners（顶层 mixed-port 被替代）。
        assert!(cfg.get("mixed-port").is_none());
        let listeners = cfg["listeners"].as_array().unwrap();
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0]["name"], "main-in");
        assert_eq!(listeners[0]["type"], "mixed");
        assert_eq!(listeners[0]["port"], 17890);
        assert_eq!(listeners[0]["listen"], "127.0.0.1");
        assert_eq!(listeners[1]["name"], "mitm-return");
        assert_eq!(listeners[1]["type"], "mixed");
        assert_eq!(listeners[1]["port"], 17891);

        // pp-mitm http 代理追加在原有 proxies 之后。
        let proxies = cfg["proxies"].as_array().unwrap();
        assert_eq!(proxies.len(), 2);
        let pp_mitm = proxies.iter().find(|p| p["name"] == "pp-mitm").unwrap();
        assert_eq!(pp_mitm["type"], "http");
        assert_eq!(pp_mitm["server"], "127.0.0.1");
        assert_eq!(pp_mitm["port"], 34567);
        assert_eq!(proxies[0]["name"], "n1");

        // 白名单规则前插（精确→DOMAIN，通配→DOMAIN-SUFFIX），原规则保留。
        // 逻辑规则语法为 `LOGIC_TYPE,((payload1),(payload2)),Proxy`：AND 与
        // 子规则之间必须有逗号，否则 mihomo 无法解析。
        let rules = cfg["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(
            rules[0],
            "AND,((IN-NAME,main-in),(DOMAIN,example.com)),pp-mitm"
        );
        assert_eq!(
            rules[1],
            "AND,((IN-NAME,main-in),(DOMAIN-SUFFIX,cdn.example.net)),pp-mitm"
        );
        assert_eq!(rules[2], "MATCH,DIRECT");
    }

    #[test]
    fn compose_mihomo_rejects_invalid_yaml() {
        let err = compose_mihomo_config("port: [unclosed", 17890, None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn compose_mihomo_rejects_non_mapping_yaml() {
        let err = compose_mihomo_config("- a\n- b", 17890, None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }
}

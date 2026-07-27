use pp_common::{PanelError, PanelResult, ProtocolType};
use serde_json::Value;

use crate::generator::ProxyNode;

/// Validate a generated sing-box config against the official JSON Schema.
/// Maps schema errors into `PanelError::Subscription`.
fn validate(config: &Value) -> PanelResult<()> {
    pp_config::validate_singbox_config(config)
        .map_err(|e| PanelError::Subscription(format!("sing-box schema: {e}")))
}

/// Protocols supported by the sing-box subscription generator.
fn is_supported(protocol: ProtocolType) -> bool {
    matches!(
        protocol,
        ProtocolType::VlessReality | ProtocolType::Hysteria2 | ProtocolType::Anytls
    )
}

/// Generate sing-box JSON subscription with outbounds array.
/// `base_config` is raw JSON template text. Supported placeholders:
///   - `<OUTBOUND_REPLACE>`  -> JSON array of generated node outbounds (spliced)
///   - `<NODE_REPLACE>`     -> JSON array of node names
///
/// Three paths:
///
///   (a) **Template path** — `base_config` contains `<OUTBOUND_REPLACE>` or
///       `<NODE_REPLACE>`:
///       Placeholders are replaced via `render_template`; generated node
///       outbounds fill `<OUTBOUND_REPLACE>`. The template must declare its
///       own selector groups / direct outbound.
///
///   (b) **Merge path** — `base_config` is plain JSON without placeholders:
///       Each generated node outbound is **appended** to
///       `base["outbounds"]` (the array is created if missing).  Base-defined
///       outbounds (direct, selectors, etc.) are preserved intact.  This
///       matches the clash.rs merge convention.
///
///   (c) **Default minimal path** — no `base_config`:
///       outbounds = node outbounds only.  When there are zero supported
///       nodes a minimal `direct` outbound is emitted so the config still
///       passes schema validation.
///
/// All three paths validate the final config against the sing-box JSON schema.
pub fn generate(nodes: &[ProxyNode], base_config: Option<&str>) -> PanelResult<String> {
    let supported: Vec<&ProxyNode> = nodes.iter().filter(|n| is_supported(n.protocol)).collect();

    let node_outbounds: Vec<_> = supported
        .iter()
        .map(|n| build_outbound(n))
        .collect::<Result<Vec<_>, _>>()?;

    let node_names: Vec<String> = supported.iter().map(|n| n.name.clone()).collect();

    // (a) Template path — placeholders present
    if let Some(base) = base_config {
        if base.contains("<OUTBOUND_REPLACE>") || base.contains("<NODE_REPLACE>") {
            let rendered = render_template(base.to_string(), &node_outbounds, &node_names)?;
            validate(&rendered)?;
            return Ok(serde_json::to_string_pretty(&rendered)?);
        }
    }

    // (b) Merge path — plain JSON base without placeholders
    if let Some(base) = base_config {
        let mut config: Value = serde_json::from_str(base).map_err(|e| {
            PanelError::Subscription(format!("failed to parse sing-box template json: {e}"))
        })?;
        // Append each node outbound to the existing outbounds array.
        // If the field is missing or not an array, create a new one.
        match config.get_mut("outbounds") {
            Some(Value::Array(arr)) => {
                arr.extend(node_outbounds);
            }
            _ => {
                config["outbounds"] = Value::Array(node_outbounds);
            }
        }
        validate(&config)?;
        return Ok(serde_json::to_string_pretty(&config)?);
    }

    // (c) Default minimal path — no base config
    let outbounds = if node_outbounds.is_empty() {
        // A valid sing-box config needs at least one outbound.
        vec![serde_json::json!({"type": "direct", "tag": "direct"})]
    } else {
        node_outbounds
    };
    let mut config = serde_json::json!({ "outbounds": [] });
    config["outbounds"] = Value::Array(outbounds);
    validate(&config)?;
    Ok(serde_json::to_string_pretty(&config)?)
}

fn render_template(
    base_str: String,
    outbounds: &[Value],
    node_names: &[String],
) -> PanelResult<Value> {
    let outbounds_json = serde_json::to_string(&outbounds)?;
    let names_json = serde_json::to_string(&node_names)?;

    // Splice array elements into the template's array position: strip the
    // outer brackets so `<OUTBOUND_REPLACE>` inside `[ ... ]` expands inline,
    // avoiding double-nested arrays (e.g. `[[{...},{...}], {"type":"direct"}]`).
    let outbounds_inner = outbounds_json
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(&outbounds_json);
    let names_inner = names_json
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(&names_json);

    let rendered = splice_placeholder(base_str, "\"<OUTBOUND_REPLACE>\"", outbounds_inner);
    let rendered = splice_placeholder(rendered, "\"<NODE_REPLACE>\"", names_inner);

    serde_json::from_str(&rendered).map_err(|e| {
        PanelError::Subscription(format!("failed to render subscription template: {e}"))
    })
}

/// Replace a quoted JSON placeholder with spliced array elements.
/// When `inner` is empty (no elements), remove the placeholder together with
/// any adjacent comma to avoid invalid JSON like `[ , {...} ]`.
fn splice_placeholder(template: String, quoted_marker: &str, inner: &str) -> String {
    if inner.trim().is_empty() {
        template
            .replace(&format!("{}, ", quoted_marker), "")
            .replace(&format!("{},", quoted_marker), "")
            .replace(&format!(", {}", quoted_marker), "")
            .replace(&format!(",{}", quoted_marker), "")
            .replace(quoted_marker, "null")
    } else {
        template.replace(quoted_marker, inner)
    }
}

fn build_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    match node.protocol {
        ProtocolType::VlessReality => build_vless_reality_outbound(node),
        ProtocolType::Hysteria2 => build_hysteria2_outbound(node),
        ProtocolType::Anytls => build_anytls_outbound(node),
        ProtocolType::VlessXhttp => Err(PanelError::Subscription(
            "sing-box does not support the vless xhttp transport".into(),
        )),
    }
}

fn build_vless_reality_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let uuid = pp_common::settings_helper::client_uuid(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing vless uuid".into()))?;

    let flow = node
        .settings
        .get("flow")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let network = node
        .settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut outbound = serde_json::json!({
        "type": "vless",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "uuid": uuid,
        "packet_encoding": "xudp",
    });

    if !flow.is_empty() {
        outbound["flow"] = serde_json::json!(flow);
    }

    if network != "tcp" {
        outbound["transport"] = serde_json::json!({
            "type": network,
        });
        if let Some(path) = node.settings.get("path").and_then(|v| v.as_str()) {
            outbound["transport"]["path"] = serde_json::json!(path);
        }
        if let Some(host) = node.settings.get("host").and_then(|v| v.as_str()) {
            outbound["transport"]["host"] = serde_json::json!(host);
        }
    }

    let public_key = node
        .settings
        .get("public_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing REALITY public_key".into()))?;
    if public_key.is_empty() {
        return Err(PanelError::Subscription(
            "missing REALITY public_key".into(),
        ));
    }
    let server_name =
        pp_common::settings_helper::first_server_name(&node.settings).unwrap_or_default();
    let short_id = pp_common::settings_helper::first_short_id(&node.settings).unwrap_or_default();
    let spider_x = node
        .settings
        .get("spider_x")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let fingerprint = node
        .settings
        .get("fingerprint")
        .and_then(|v| v.as_str())
        .unwrap_or("chrome");

    let mut reality = serde_json::json!({
        "enabled": true,
        "public_key": public_key,
        "short_id": short_id,
    });
    if !spider_x.is_empty() {
        reality["spider_x"] = serde_json::json!(spider_x);
    }

    outbound["tls"] = serde_json::json!({
        "enabled": true,
        "server_name": server_name,
        "utls": {
            "enabled": true,
            "fingerprint": fingerprint,
        },
        "reality": reality,
    });

    Ok(outbound)
}

fn build_hysteria2_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing hysteria2 password".into()))?;

    let server_name = node
        .tls
        .as_ref()
        .and_then(|t| t.get("serverName"))
        .and_then(|v| v.as_str())
        .or_else(|| node.settings.get("sni").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let skip_verify = node
        .settings
        .get("skip_cert_verify")
        .and_then(|v| v.as_bool())
        .unwrap_or(!pp_common::settings_helper::tls_has_real_certificate(
            node.tls.as_ref(),
        ));

    let mut outbound = serde_json::json!({
        "type": "hysteria2",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "password": password,
        "tls": {
            "enabled": true,
            "server_name": server_name,
            "insecure": skip_verify,
            "alpn": ["h3"],
        },
    });

    if let Some(up) = node.settings.get("up_mbps").and_then(|v| v.as_u64()) {
        outbound["up_mbps"] = serde_json::json!(up);
    }
    if let Some(down) = node.settings.get("down_mbps").and_then(|v| v.as_u64()) {
        outbound["down_mbps"] = serde_json::json!(down);
    }
    if let Some(obfs_type) = node.settings.get("obfs_type").and_then(|v| v.as_str()) {
        if obfs_type != "none" {
            if let Some(obfs_password) = node.settings.get("obfs_password").and_then(|v| v.as_str())
            {
                outbound["obfs"] = serde_json::json!({
                    "type": obfs_type,
                    "password": obfs_password,
                });
            }
        }
    }

    Ok(outbound)
}

fn build_anytls_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing anytls password".into()))?;

    let server_name = node
        .tls
        .as_ref()
        .and_then(|t| t.get("serverName"))
        .and_then(|v| v.as_str())
        .or_else(|| node.settings.get("sni").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let skip_verify = node
        .settings
        .get("skip_cert_verify")
        .and_then(|v| v.as_bool())
        .unwrap_or(!pp_common::settings_helper::tls_has_real_certificate(
            node.tls.as_ref(),
        ));

    Ok(serde_json::json!({
        "type": "anytls",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "password": password,
        "tls": {
            "enabled": true,
            "server_name": server_name,
            "insecure": skip_verify,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::ProxyNode;
    use serde_json::json;

    fn reality_node() -> ProxyNode {
        ProxyNode {
            name: "test-reality".into(),
            protocol: ProtocolType::VlessReality,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "id": "",
                "clients": [{"id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4"}],
                "flow": "xtls-rprx-vision",
                "public_key": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                "server_names": ["example.com", "www.example.com"],
                "short_id": ["0123456789abcdef"],
                "fingerprint": "chrome"
            }),
            tls: None,
        }
    }

    #[test]
    fn vless_reality_outbound_has_reality_tls() {
        let outbound = build_vless_reality_outbound(&reality_node()).unwrap();
        assert_eq!(outbound["type"], "vless");
        assert_eq!(outbound["server"], "1.2.3.4");
        assert_eq!(outbound["server_port"], 443);
        assert_eq!(outbound["flow"], "xtls-rprx-vision");
        assert_eq!(outbound["packet_encoding"], "xudp");
        assert_eq!(outbound["tls"]["enabled"], true);
        assert_eq!(outbound["tls"]["server_name"], "example.com");
        assert_eq!(
            outbound["tls"]["reality"]["public_key"],
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
        );
        assert_eq!(outbound["tls"]["reality"]["short_id"], "0123456789abcdef");
        assert_eq!(outbound["tls"]["utls"]["fingerprint"], "chrome");
    }

    #[test]
    fn hysteria2_outbound_insecure_when_no_real_certificate() {
        let node = ProxyNode {
            name: "test-hy2".into(),
            protocol: ProtocolType::Hysteria2,
            server: "1.2.3.4".into(),
            port: 8443,
            settings: json!({
                "clients": [{"password": "hy2-secret"}],
            }),
            tls: Some(json!({ "serverName": "hy2.example.com" })),
        };
        let outbound = build_hysteria2_outbound(&node).unwrap();
        assert_eq!(outbound["tls"]["insecure"], true);
        assert_eq!(outbound["tls"]["alpn"], json!(["h3"]));
    }

    #[test]
    fn hysteria2_outbound_verifies_when_acme_domain_present() {
        let node = ProxyNode {
            name: "test-hy2".into(),
            protocol: ProtocolType::Hysteria2,
            server: "1.2.3.4".into(),
            port: 8443,
            settings: json!({
                "clients": [{"password": "hy2-secret"}],
            }),
            tls: Some(json!({ "domain": "hy2.example.com" })),
        };
        let outbound = build_hysteria2_outbound(&node).unwrap();
        assert_eq!(outbound["tls"]["insecure"], false);
        assert_eq!(outbound["tls"]["alpn"], json!(["h3"]));
    }

    #[test]
    fn vless_reality_outbound_requires_public_key() {
        let mut node = reality_node();
        node.settings["public_key"] = json!("");
        let err = build_vless_reality_outbound(&node).unwrap_err();
        assert!(err.to_string().contains("public_key"));
    }

    // --- Template rendering tests ---

    /// A minimal template that references both placeholders, mimicking the
    /// structure of the built-in template (groups declared by the template
    /// itself).
    const BUILTIN_TEMPLATE: &str = r#"{
        "outbounds": [
            "<OUTBOUND_REPLACE>",
            {
                "type": "selector",
                "tag": "proxy",
                "outbounds": ["<NODE_REPLACE>"]
            },
            {
                "type": "selector",
                "tag": "global",
                "outbounds": ["proxy", "<NODE_REPLACE>", "direct"]
            },
            {
                "type": "direct",
                "tag": "direct"
            }
        ]
    }"#;

    #[test]
    fn render_template_splices_outbound_elements_not_array() {
        let outbounds = vec![
            json!({"type": "vless", "tag": "node1", "server": "1.2.3.4"}),
            json!({"type": "hysteria2", "tag": "node2", "server": "5.6.7.8"}),
        ];
        let names = vec!["node1".into(), "node2".into()];
        let result = render_template(BUILTIN_TEMPLATE.to_string(), &outbounds, &names).unwrap();
        let outbounds_arr = result["outbounds"].as_array().unwrap();
        // 2 nodes + proxy + global + direct = 5
        assert_eq!(outbounds_arr.len(), 5);
        // First element must be an object, NOT an array (no double-nesting)
        assert!(outbounds_arr[0].is_object());
        assert_eq!(outbounds_arr[0]["tag"], "node1");
        assert_eq!(outbounds_arr[1]["tag"], "node2");
        // proxy selector
        assert_eq!(outbounds_arr[2]["tag"], "proxy");
        let proxy_obs: Vec<&str> = outbounds_arr[2]["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(proxy_obs, vec!["node1", "node2"]);
        // global selector
        assert_eq!(outbounds_arr[3]["tag"], "global");
        let global_obs: Vec<&str> = outbounds_arr[3]["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(global_obs, vec!["proxy", "node1", "node2", "direct"]);
        // direct
        assert_eq!(outbounds_arr[4]["type"], "direct");
    }

    #[test]
    fn render_template_empty_outbounds_produces_valid_json() {
        let outbounds: Vec<Value> = vec![];
        let names: Vec<String> = vec![];
        let result = render_template(BUILTIN_TEMPLATE.to_string(), &outbounds, &names).unwrap();
        let outbounds_arr = result["outbounds"].as_array().unwrap();
        // With 0 nodes: proxy (with ["null"] from placeholder fallback),
        // global (with ["proxy","direct"]), and direct
        assert_eq!(outbounds_arr.len(), 3);
        assert_eq!(outbounds_arr[0]["tag"], "proxy");
        assert_eq!(outbounds_arr[1]["tag"], "global");
        assert_eq!(outbounds_arr[2]["type"], "direct");
    }

    // --- Full generate() integration tests ---

    fn two_reality_nodes() -> Vec<ProxyNode> {
        vec![
            ProxyNode {
                name: "tokyo".into(),
                protocol: ProtocolType::VlessReality,
                server: "10.0.0.1".into(),
                port: 443,
                settings: json!({
                    "clients": [{"id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4"}],
                    "flow": "xtls-rprx-vision",
                    "public_key": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                    "server_names": ["tokyo.example.com"],
                    "short_id": ["01"],
                    "fingerprint": "chrome",
                }),
                tls: None,
            },
            ProxyNode {
                name: "singapore".into(),
                protocol: ProtocolType::VlessReality,
                server: "10.0.0.2".into(),
                port: 443,
                settings: json!({
                    "clients": [{"id": "b4b4b4b4-b4b4-b4b4-b4b4-b4b4b4b4b4b4"}],
                    "flow": "xtls-rprx-vision",
                    "public_key": "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                    "server_names": ["sg.example.com"],
                    "short_id": ["02"],
                    "fingerprint": "chrome",
                }),
                tls: None,
            },
        ]
    }

    #[test]
    fn generate_default_path_contains_only_node_outbounds() {
        let nodes = two_reality_nodes();
        let result = generate(&nodes, None).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        // Only the 2 node outbounds — no proxy/global/direct
        assert_eq!(outbounds.len(), 2);

        let tags: Vec<&str> = outbounds
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        assert_eq!(tags, vec!["tokyo", "singapore"]);
    }

    #[test]
    fn generate_default_path_zero_supported_nodes_fallback_to_direct() {
        let nodes: Vec<ProxyNode> = vec![];
        let result = generate(&nodes, None).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        // Fallback direct outbound keeps the config valid
        assert_eq!(outbounds.len(), 1);
        assert_eq!(outbounds[0]["type"], "direct");
        assert_eq!(outbounds[0]["tag"], "direct");
    }

    #[test]
    fn generate_builtin_template_render_passes_schema() {
        // Full template matching the built-in template structure
        // with route.final = "proxy" referencing the proxy selector.
        let template = r#"{
            "outbounds": [
                "<OUTBOUND_REPLACE>",
                {
                    "type": "selector",
                    "tag": "proxy",
                    "outbounds": ["<NODE_REPLACE>"]
                },
                {
                    "type": "selector",
                    "tag": "global",
                    "outbounds": ["proxy", "<NODE_REPLACE>", "direct"]
                },
                {
                    "type": "direct",
                    "tag": "direct"
                }
            ],
            "route": { "final": "proxy" }
        }"#;

        let nodes = two_reality_nodes();
        let result = generate(&nodes, Some(template)).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        assert_eq!(outbounds.len(), 5);
        assert_eq!(outbounds[0]["tag"], "tokyo");
        assert_eq!(outbounds[1]["tag"], "singapore");
        assert_eq!(outbounds[2]["tag"], "proxy");
        assert_eq!(outbounds[3]["tag"], "global");
        assert_eq!(outbounds[4]["tag"], "direct");

        // proxy selector lists both node names
        let proxy_obs: Vec<&str> = outbounds[2]["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(proxy_obs, vec!["tokyo", "singapore"]);

        // global selector: proxy + nodes + direct
        let global_obs: Vec<&str> = outbounds[3]["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(global_obs, vec!["proxy", "tokyo", "singapore", "direct"]);

        assert_eq!(config["route"]["final"], "proxy");
    }

    #[test]
    fn generate_merge_path_appends_to_base_outbounds() {
        // Base config has its own direct + proxy-selector with ["x"].
        let template = r#"{
            "route": { "final": "proxy" },
            "outbounds": [
                { "type": "direct", "tag": "direct" },
                { "type": "selector", "tag": "proxy", "outbounds": ["x"] }
            ]
        }"#;

        let nodes = two_reality_nodes();
        let result = generate(&nodes, Some(template)).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        // Base: direct + proxy‑selector = 2; appended: tokyo + singapore = 2; total = 4
        assert_eq!(outbounds.len(), 4);

        // Base outbounds preserved at their original positions
        assert_eq!(outbounds[0]["tag"], "direct");
        assert_eq!(outbounds[0]["type"], "direct");
        assert_eq!(outbounds[1]["tag"], "proxy");
        assert_eq!(outbounds[1]["type"], "selector");

        // Node outbounds appended after base outbounds
        assert_eq!(outbounds[2]["tag"], "tokyo");
        assert_eq!(outbounds[3]["tag"], "singapore");
    }
}

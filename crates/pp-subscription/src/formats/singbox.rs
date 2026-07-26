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

/// Build the two selector group outbounds (proxy / global) that sing-box
/// templates such as the built-in one reference via `route.final = "proxy"`.
/// Returns an empty vec when `node_names` is empty (avoids empty selector
/// outbound arrays, which sing-box rejects).
fn build_selectors(node_names: &[String]) -> Vec<Value> {
    if node_names.is_empty() {
        return vec![];
    }

    let proxy = serde_json::json!({
        "type": "selector",
        "tag": "proxy",
        "outbounds": node_names,
    });

    let mut global_outbounds: Vec<String> = Vec::with_capacity(node_names.len() + 2);
    global_outbounds.push("proxy".to_string());
    global_outbounds.extend(node_names.iter().cloned());
    global_outbounds.push("direct".to_string());

    let global = serde_json::json!({
        "type": "selector",
        "tag": "global",
        "outbounds": global_outbounds,
    });

    vec![proxy, global]
}

/// Generate sing-box JSON subscription with outbounds array.
/// `base_config` is raw JSON template text. Supported placeholders:
///   - `<OUTBOUND_REPLACE>`  -> JSON array of generated outbounds
///   - `<NODE_REPLACE>`     -> JSON array of node names
///
/// Group outbounds (`proxy` / `global` selectors) are automatically appended
/// so that templates with `route.final = "proxy"` resolve correctly.
/// When there are zero supported nodes the selectors are skipped.
pub fn generate(nodes: &[ProxyNode], base_config: Option<&str>) -> PanelResult<String> {
    let supported: Vec<&ProxyNode> = nodes.iter().filter(|n| is_supported(n.protocol)).collect();

    let node_outbounds: Vec<_> = supported
        .iter()
        .map(|n| build_outbound(n))
        .collect::<Result<Vec<_>, _>>()?;

    let node_names: Vec<String> = supported.iter().map(|n| n.name.clone()).collect();

    let has_nodes = !supported.is_empty();
    let selectors = build_selectors(&node_names);
    let direct = serde_json::json!({"type": "direct", "tag": "direct"});

    // Template path (a): outbounds = node outbounds + selectors; direct comes from template text.
    let template_outbounds: Vec<Value> = node_outbounds
        .iter()
        .cloned()
        .chain(selectors.iter().cloned())
        .collect();

    // Merge & default paths (b, c): outbounds = node outbounds + selectors + direct.
    let full_outbounds: Vec<Value> = if has_nodes {
        node_outbounds
            .iter()
            .cloned()
            .chain(selectors.iter().cloned())
            .chain(std::iter::once(direct))
            .collect()
    } else {
        vec![direct]
    };

    if let Some(base) = base_config {
        if base.contains("<OUTBOUND_REPLACE>") || base.contains("<NODE_REPLACE>") {
            let rendered = render_template(base.to_string(), &template_outbounds, &node_names)?;
            validate(&rendered)?;
            return Ok(serde_json::to_string_pretty(&rendered)?);
        }

        let mut config: Value = serde_json::from_str(base).map_err(|e| {
            PanelError::Subscription(format!("failed to parse sing-box template json: {e}"))
        })?;
        config["outbounds"] = serde_json::Value::Array(full_outbounds);
        validate(&config)?;
        return Ok(serde_json::to_string_pretty(&config)?);
    }

    let mut config = serde_json::json!({ "outbounds": [] });
    config["outbounds"] = serde_json::Value::Array(full_outbounds);
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

    // --- Template rendering tests (Task A) ---

    const BUILTIN_TEMPLATE: &str = r#"{
        "outbounds": [
            "<OUTBOUND_REPLACE>",
            { "type": "direct", "tag": "direct" }
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
        // Should have 3 elements: 2 spliced + 1 direct
        assert_eq!(outbounds_arr.len(), 3);
        // First element must be an object, NOT an array (no double-nesting)
        assert!(outbounds_arr[0].is_object());
        assert_eq!(outbounds_arr[0]["tag"], "node1");
        assert_eq!(outbounds_arr[1]["tag"], "node2");
        assert_eq!(outbounds_arr[2]["type"], "direct");
    }

    #[test]
    fn render_template_empty_outbounds_produces_valid_json() {
        let outbounds: Vec<Value> = vec![];
        let names: Vec<String> = vec![];
        let result = render_template(BUILTIN_TEMPLATE.to_string(), &outbounds, &names).unwrap();
        let outbounds_arr = result["outbounds"].as_array().unwrap();
        // With 0 nodes, only the direct outbound remains
        assert_eq!(outbounds_arr.len(), 1);
        assert_eq!(outbounds_arr[0]["type"], "direct");
    }

    // --- Group selector helper ---

    #[test]
    fn build_selectors_empty_input_returns_empty() {
        let names: Vec<String> = vec![];
        let sels = build_selectors(&names);
        assert!(sels.is_empty());
    }

    #[test]
    fn build_selectors_two_nodes_creates_proxy_and_global() {
        let names = vec!["alpha".into(), "beta".into()];
        let sels = build_selectors(&names);
        assert_eq!(sels.len(), 2);

        // proxy selector
        let proxy = &sels[0];
        assert_eq!(proxy["type"], "selector");
        assert_eq!(proxy["tag"], "proxy");
        let proxy_obs = proxy["outbounds"].as_array().unwrap();
        assert_eq!(proxy_obs.len(), 2);
        assert_eq!(proxy_obs[0], "alpha");
        assert_eq!(proxy_obs[1], "beta");

        // global selector
        let global = &sels[1];
        assert_eq!(global["type"], "selector");
        assert_eq!(global["tag"], "global");
        let global_obs = global["outbounds"].as_array().unwrap();
        assert_eq!(global_obs.len(), 4);
        assert_eq!(global_obs[0], "proxy");
        assert_eq!(global_obs[1], "alpha");
        assert_eq!(global_obs[2], "beta");
        assert_eq!(global_obs[3], "direct");
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
    fn generate_default_path_includes_selectors_and_direct() {
        let nodes = two_reality_nodes();
        let result = generate(&nodes, None).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        // tokyo, singapore, proxy, global, direct = 5
        assert_eq!(outbounds.len(), 5);

        let tags: Vec<&str> = outbounds
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        assert_eq!(
            tags,
            vec!["tokyo", "singapore", "proxy", "global", "direct"]
        );

        // proxy selector lists both node names
        let proxy = &outbounds[2];
        assert_eq!(proxy["type"], "selector");
        let proxy_obs: Vec<&str> = proxy["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(proxy_obs, vec!["tokyo", "singapore"]);
    }

    #[test]
    fn generate_default_path_zero_supported_nodes_skips_selectors() {
        let nodes: Vec<ProxyNode> = vec![];
        let result = generate(&nodes, None).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        // Only direct should be present
        assert_eq!(outbounds.len(), 1);
        assert_eq!(outbounds[0]["type"], "direct");
    }

    #[test]
    fn generate_builtin_template_render_passes_schema() {
        // Full builtin template with route.final = "proxy" + placeholders
        let template = r#"{
            "outbounds": [
                "<OUTBOUND_REPLACE>",
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "proxy" }
        }"#;

        let nodes = two_reality_nodes();
        let result = generate(&nodes, Some(template)).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        // Schema validation — already called inside generate(), but verify
        // explicitly that proxy selector + route.final = "proxy" is valid.
        let outbounds = config["outbounds"].as_array().unwrap();
        assert_eq!(outbounds.len(), 5);
        assert_eq!(outbounds[0]["tag"], "tokyo");
        assert_eq!(outbounds[1]["tag"], "singapore");
        assert_eq!(outbounds[2]["tag"], "proxy");
        assert_eq!(outbounds[3]["tag"], "global");
        assert_eq!(outbounds[4]["tag"], "direct");
        assert_eq!(config["route"]["final"], "proxy");
    }

    #[test]
    fn generate_merge_path_replaces_outbounds_wholesale() {
        let template = r#"{
            "route": { "final": "proxy" },
            "outbounds": [
                { "type": "direct", "tag": "direct" }
            ]
        }"#;

        let nodes = two_reality_nodes();
        let result = generate(&nodes, Some(template)).unwrap();
        let config: Value = serde_json::from_str(&result).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        assert_eq!(outbounds.len(), 5);
        // Proxy selector present
        let proxy = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
        assert_eq!(proxy["type"], "selector");
    }
}

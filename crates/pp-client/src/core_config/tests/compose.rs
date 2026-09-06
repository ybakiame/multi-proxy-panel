use super::*;
use pp_common::PanelError;
use serde_json::{Value, json};

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
        hostnames: vec![
            "example.com".to_string(),
            "*.cdn.example.net".to_string(),
            "-excluded.example.org".to_string(),
        ],
    };
    let cfg = compose_singbox_config(&sub, 17890, Some(chain)).unwrap();

    // Dual mixed inbounds: main entry + return entry.
    let inbounds = cfg["inbounds"].as_array().unwrap();
    assert_eq!(inbounds.len(), 2);
    assert_eq!(inbounds[0]["type"], "mixed");
    assert_eq!(inbounds[0]["tag"], "main-in");
    assert_eq!(inbounds[0]["listen"], "127.0.0.1");
    assert_eq!(inbounds[0]["listen_port"], 17890);
    assert_eq!(inbounds[1]["tag"], "mitm-return");
    assert_eq!(inbounds[1]["listen_port"], 17891);

    // pp-mitm http outbound appended after existing outbounds.
    let outbounds = cfg["outbounds"].as_array().unwrap();
    assert_eq!(outbounds.len(), 2);
    assert_eq!(outbounds[1]["tag"], "pp-mitm");
    assert_eq!(outbounds[1]["type"], "http");
    assert_eq!(outbounds[1]["server"], "127.0.0.1");
    assert_eq!(outbounds[1]["server_port"], 34567);

    // Route rules prepended: inbound matches main entry, exact/suffix分流, final preserved;
    // `-excluded.example.org` is exclusion, no core routing rule generated.
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

/// sing-box 1.12+ requires `route.default_domain_resolver` to point to a declared tag of
/// DNS server, otherwise real sing-box rejects config (legacy resolver missing).
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
    // Existing tagged server is not rewritten.
    assert_eq!(cfg["dns"]["servers"][0]["tag"], "dns1");
}

#[test]
fn compose_singbox_generates_tag_for_tagless_dns_server() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "dns": { "servers": [{ "type": "udp", "server": "1.1.1.1" }] }
    });
    let cfg = compose_singbox_config(&sub, 17890, None).unwrap();

    // Tagless server gets auto tag, resolver points to it.
    assert_eq!(cfg["dns"]["servers"][0]["tag"], "dns-0");
    assert_eq!(
        cfg["route"]["default_domain_resolver"],
        json!({ "server": "dns-0" })
    );
}

#[test]
fn compose_singbox_keeps_existing_resolver_and_skips_when_no_dns() {
    // Subscription already explicitly declares resolver -> do not override.
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

    // No dns -> no resolver injected.
    let bare = json!({ "outbounds": [{ "type": "direct", "tag": "direct" }] });
    let cfg = compose_singbox_config(&bare, 17890, None).unwrap();
    assert!(cfg["route"].get("default_domain_resolver").is_none());
}

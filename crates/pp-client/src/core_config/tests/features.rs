use super::*;
use pp_common::CoreType;
use serde_json::json;

fn singbox_features() -> PanelFeatures {
    PanelFeatures {
        tun_enabled: true,
        tun_stack: "mixed".to_string(),
        tun_auto_route: true,
        clash_api_enabled: true,
        clash_api_port: 9090,
        clash_api_secret: "sekret".to_string(),
        clash_api_ui: "zashboard".to_string(),
        rule_mode: "rule".to_string(),
    }
}

#[test]
fn apply_singbox_panel_features_injects_tun_and_clash_api() {
    let sub = json!({
        "inbounds": [{ "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 17890 }],
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());

    // tun inbound appended: tag / address / mtu / auto_route / stack.
    let inbounds = cfg["inbounds"].as_array().unwrap();
    let tun = inbounds
        .iter()
        .find(|i| i["type"] == "tun")
        .expect("should inject tun inbound");
    assert_eq!(tun["tag"], "tun-in");
    assert_eq!(tun["address"], "172.19.0.1/30");
    assert_eq!(tun["mtu"], 9000);
    assert_eq!(tun["auto_route"], true);
    assert_eq!(tun["stack"], "mixed");
    assert_eq!(inbounds.len(), 2, "mixed-in retained + tun-in appended");

    // experimental.clash_api injected (with secret).
    assert_eq!(
        cfg["experimental"]["clash_api"]["external_controller"],
        "127.0.0.1:9090"
    );
    assert_eq!(cfg["experimental"]["clash_api"]["secret"], "sekret");
}

#[test]
fn apply_singbox_panel_features_overrides_template_tun() {
    // Template/override already has tun inbound and experimental.clash_api -> replaced by settings.
    let sub = json!({
        "inbounds": [
            { "type": "tun", "tag": "tun-in", "address": "10.0.0.1/24", "mtu": 1500, "auto_route": false, "stack": "system" },
            { "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 17890 }
        ],
        "experimental": {
            "clash_api": {
                "external_controller": "0.0.0.0:60000",
                "external_ui": "yacd-dir",
                "external_ui_download_url": "https://old.example/panel.zip",
                "secret": "old"
            }
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());

    let inbounds = cfg["inbounds"].as_array().unwrap();
    let tun: Vec<_> = inbounds.iter().filter(|i| i["type"] == "tun").collect();
    assert_eq!(
        tun.len(),
        1,
        "template tun replaced, only one tun inbound kept"
    );
    assert_eq!(tun[0]["address"], "172.19.0.1/30");
    assert_eq!(tun[0]["mtu"], 9000);
    assert_eq!(tun[0]["stack"], "mixed");

    // experimental.clash_api wholesale replacement: template's external_ui and download URL
    // also overridden by settings (external_ui=ui-zashboard + selected download URL), remaining
    // experimental fields (if any) preserved.
    assert_eq!(
        cfg["experimental"]["clash_api"]["external_controller"],
        "127.0.0.1:9090"
    );
    assert_eq!(cfg["experimental"]["clash_api"]["secret"], "sekret");
    assert_eq!(
        cfg["experimental"]["clash_api"]["external_ui"],
        "ui-zashboard"
    );
    assert_eq!(
        cfg["experimental"]["clash_api"]["external_ui_download_url"],
        "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip",
        "template old external_ui / download URL should be overridden by settings"
    );
}

/// Android (libbox / VpnService takes over traffic) tun inbound must contain libbox compatible fields:
/// type / tag / address / mtu / auto_route / stack / strict_route; no desktop-only
/// fields (interface_name / fd), and no inbound-level `sniff` removed since sing-box 1.13
/// (`check -c` will reject). Desktop keeps original field set.
#[test]
fn build_singbox_tun_inbound_matches_libbox_field_set_on_android() {
    let android_tun = build_singbox_tun_inbound(&singbox_features(), true);
    assert_eq!(android_tun["type"], "tun");
    assert_eq!(android_tun["tag"], "tun-in");
    assert_eq!(android_tun["address"], "172.19.0.1/30");
    assert_eq!(android_tun["mtu"], 9000);
    assert_eq!(android_tun["auto_route"], true);
    assert_eq!(android_tun["stack"], "mixed");
    assert_eq!(android_tun["strict_route"], true);
    // Desktop-only fields not injected (libbox resolves interface name via getTunnelName(fd)).
    assert!(android_tun.get("interface_name").is_none());
    assert!(android_tun.get("fd").is_none());
    // sing-box 1.13+ rejects inbound-level sniff legacy field.
    assert!(android_tun.get("sniff").is_none());

    let desktop_tun = build_singbox_tun_inbound(&singbox_features(), false);
    assert_eq!(desktop_tun["type"], "tun");
    assert_eq!(desktop_tun["stack"], "mixed");
    assert!(
        desktop_tun.get("strict_route").is_none(),
        "desktop tun inbound should not contain Android-only strict_route: {desktop_tun}"
    );
}

/// Android composed config (tun_enabled=true with libbox field set) must pass real
/// sing-box `check -c` (equivalent to `singbox_tun_clash_api_passes_real_singbox_check`
/// but through Android field set branch).
#[test]
fn android_tun_inbound_passes_real_singbox_check() {
    let Some(bin) = sing_box_binary() else {
        return;
    };
    let sub = json!({
        "outbounds": [
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "direct" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    // Inject clash_api (Android frontend can also enable); tun handled separately via Android field set.
    let clash_only = PanelFeatures {
        tun_enabled: false,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, CoreType::SingBox, &clash_only);
    // Android field set tun inbound: strict_route.
    let tun = build_singbox_tun_inbound(&singbox_features(), true);
    cfg["inbounds"].as_array_mut().unwrap().push(tun);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
    let out = std::process::Command::new(&bin)
        .args(["check", "-c"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "sing-box check failed (android tun field set): {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------- Android explicit DNS injection (system resolver unavailable after VpnService takeover) ----------

#[test]
fn inject_android_dns_sets_explicit_dns_with_actual_selector_detour() {
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["n1", "direct"], "default": "n1" },
            { "type": "vless", "tag": "n1", "server": "proxy-panel.ybakiame.net", "server_port": 443 },
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);

    // remote uses actual selector tag (not hardcoded), local has no detour (direct by default).
    assert_eq!(cfg["dns"]["servers"][0]["tag"], "remote");
    assert_eq!(cfg["dns"]["servers"][0]["detour"], "proxy");
    assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
    assert!(cfg["dns"]["servers"][1].get("detour").is_none());
    assert_eq!(cfg["dns"]["rules"], json!([]));
    assert_eq!(cfg["dns"]["final"], "remote");
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
    // sing-box 1.12+ requires explicit default_domain_resolver (pointing to first tagged server).
    assert_eq!(
        cfg["route"]["default_domain_resolver"],
        json!({ "server": "remote" })
    );
    // Outbounds with server field get domain_resolver -> local (proxy server domain direct resolve,
    // avoid remote loopback); selector outbound has no server field -> not injected.
    let outbounds = cfg["outbounds"].as_array().unwrap();
    let vless = outbounds.iter().find(|o| o["tag"] == "n1").unwrap();
    assert_eq!(vless["domain_resolver"], json!({ "server": "local" }));
    let selector = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
    assert!(
        selector.get("domain_resolver").is_none(),
        "selector outbound should not get domain_resolver injected"
    );
}

/// Outbound already has `domain_resolver` (subscription/template explicit config) -> not overridden.
#[test]
fn inject_android_dns_keeps_existing_outbound_domain_resolver() {
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["n1"], "default": "n1" },
            {
                "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                "domain_resolver": { "server": "custom" }
            }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);

    let outbounds = cfg["outbounds"].as_array().unwrap();
    let vless = outbounds.iter().find(|o| o["tag"] == "n1").unwrap();
    assert_eq!(
        vless["domain_resolver"],
        json!({ "server": "custom" }),
        "subscription/template explicit domain_resolver should not be overridden"
    );
}

/// Outbounds without `server` field (selector/urltest/direct) do not get domain_resolver injected.
#[test]
fn inject_android_dns_does_not_inject_domain_resolver_without_server() {
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["auto", "direct"], "default": "auto" },
            { "type": "urltest", "tag": "auto", "outbounds": ["direct"], "url": "https://www.gstatic.com/generate_204", "interval": "5m" },
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);

    for outbound in cfg["outbounds"].as_array().unwrap() {
        assert!(
            outbound.get("domain_resolver").is_none(),
            "outbound without server field should not get domain_resolver injected: {outbound}"
        );
    }
}

/// Without selector, detour target falls back to route.final; route.final = "direct" (empty direct
/// outbound) -> remote detour omitted (sing-box rejects detour to empty direct outbound), DNS
/// still injected.
#[test]
fn inject_android_dns_falls_back_to_route_final_when_no_selector() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": { "final": "direct" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);

    assert_eq!(cfg["dns"]["servers"][0]["tag"], "remote");
    assert!(
        cfg["dns"]["servers"][0].get("detour").is_none(),
        "remote should omit detour when route.final points to empty direct outbound"
    );
    assert_eq!(
        cfg["route"]["default_domain_resolver"],
        json!({ "server": "remote" })
    );
}

/// Subscription mode composed config has no direct outbound: local DNS has no detour (omitted = direct by default),
/// no direct outbound created, outbounds kept as-is.
#[test]
fn inject_android_dns_leaves_outbounds_untouched_when_no_direct() {
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["n1"], "default": "n1" },
            { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);

    // local DNS server has no detour (direct by default), and no direct outbound created.
    assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
    assert!(cfg["dns"]["servers"][1].get("detour").is_none());
    let outbounds = cfg["outbounds"].as_array().unwrap();
    assert!(
        outbounds.iter().all(|o| o["type"] != "direct"),
        "should not create direct outbound when none exists"
    );
}

/// Existing direct outbound with custom tag: local still has no detour (does not reference any direct outbound),
/// outbounds not modified.
#[test]
fn inject_android_dns_leaves_existing_direct_outbound_untouched() {
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["dns-direct"], "default": "dns-direct" },
            { "type": "direct", "tag": "dns-direct" }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);

    // local has no detour, does not reference/modify existing direct outbound.
    assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
    assert!(cfg["dns"]["servers"][1].get("detour").is_none());
    let direct_count = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["type"] == "direct")
        .count();
    assert_eq!(
        direct_count, 1,
        "should not modify existing direct outbound"
    );
}

/// route.final points to direct outbound with extra config keys (non-empty direct), can be used as detour
/// target: without selector scenario, remote detour kept as route.final's tag.
#[test]
fn inject_android_dns_keeps_detour_for_non_empty_direct() {
    let sub = json!({
        "outbounds": [
            { "type": "direct", "tag": "direct", "override_address": "1.2.3.4" }
        ],
        "route": { "final": "direct" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);

    assert_eq!(cfg["dns"]["servers"][0]["tag"], "remote");
    assert_eq!(
        cfg["dns"]["servers"][0]["detour"], "direct",
        "direct outbound with extra config keys is a valid detour target, detour should be kept"
    );
}

/// Neither selector nor route.final: cannot determine detour -> skip injection (do not produce invalid config).
#[test]
fn inject_android_dns_skips_when_no_outbound_hint() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    assert!(
        cfg.get("route").is_none(),
        "sub-config has no route no final"
    );
    inject_android_dns(&mut cfg);

    assert!(cfg.get("dns").is_none());
}

/// Android composed config (tun inbound + explicit DNS injection) must pass real `sing-box check`.
#[test]
fn android_config_with_injected_dns_passes_real_singbox_check() {
    let Some(bin) = sing_box_binary() else {
        return;
    };
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["auto", "direct"], "default": "auto" },
            { "type": "urltest", "tag": "auto", "outbounds": ["direct"], "url": "https://www.gstatic.com/generate_204", "interval": "5m" },
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    // Simulate Android panel injection path: tun inbound (Android field set) + clash_api + explicit DNS.
    apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());
    inject_android_dns(&mut cfg);

    // Composed config main selector tag is `proxy` (singbox_template fixed group name).
    assert_eq!(cfg["dns"]["servers"][0]["detour"], "proxy");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
    let out = std::process::Command::new(&bin)
        .args(["check", "-c"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "sing-box check failed (android dns injection): {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Subscription mode composed config completely lacks direct outbound: local DNS omits detour (direct by default),
/// no direct outbound created, config must pass real `sing-box check`.
///
/// Note: `detour to an empty direct outbound makes no sense` is a startup stage error,
/// `sing-box check` (static validation) cannot cover it, local assertion is the main defense.
#[test]
fn android_config_without_direct_outbound_passes_real_singbox_check() {
    let Some(bin) = sing_box_binary() else {
        return;
    };
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["auto"], "default": "auto" },
            { "type": "urltest", "tag": "auto", "outbounds": ["n1"], "url": "https://www.gstatic.com/generate_204", "interval": "5m" },
            { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());
    inject_android_dns(&mut cfg);

    // No direct outbound -> local DNS has no detour, and outbounds contains no created direct outbound.
    assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
    assert!(cfg["dns"]["servers"][1].get("detour").is_none());
    assert!(
        !cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["type"] == "direct"),
        "should not create direct outbound"
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
    let out = std::process::Command::new(&bin)
        .args(["check", "-c"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "sing-box check failed (android dns injection, no direct outbound): {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn apply_singbox_panel_features_disabled_leaves_config_untouched() {
    let sub = json!({
        "inbounds": [{ "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 17890 }],
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let disabled = PanelFeatures {
        tun_enabled: false,
        ..singbox_features()
    };
    let disabled = PanelFeatures {
        tun_stack: String::new(),
        clash_api_enabled: false,
        ..disabled
    };
    apply_panel_features(&mut cfg, CoreType::SingBox, &disabled);

    assert!(
        !cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "tun")
    );
    assert!(cfg.get("experimental").is_none());
}

#[test]
fn apply_mihomo_panel_features_injects_tun_and_external_controller() {
    let yaml =
        "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    apply_panel_features(&mut cfg, CoreType::Mihomo, &singbox_features());

    // tun map injection.
    assert_eq!(cfg["tun"]["enable"], true);
    assert_eq!(cfg["tun"]["stack"], "mixed");
    assert_eq!(cfg["tun"]["auto-route"], true);
    assert_eq!(cfg["tun"]["auto-detect-interface"], true);
    assert_eq!(cfg["tun"]["dns-hijack"], json!(["any:53"]));
    // external-controller + secret injection.
    assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
    assert_eq!(cfg["secret"], "sekret");
}

#[test]
fn apply_mihomo_panel_features_overrides_and_omits_empty_secret() {
    let yaml = r#"
mixed-port: 17890
tun:
  enable: false
  stack: system
external-controller: 0.0.0.0:60000
proxies:
  - name: n1
    type: direct
rules:
  - MATCH,DIRECT
"#;
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    let features = PanelFeatures {
        tun_stack: "gvisor".to_string(),
        tun_auto_route: false,
        clash_api_secret: String::new(), // empty secret -> omit this key
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, CoreType::Mihomo, &features);

    // Template tun / external-controller replaced by settings.
    assert_eq!(cfg["tun"]["enable"], true);
    assert_eq!(cfg["tun"]["stack"], "gvisor");
    assert_eq!(cfg["tun"]["auto-route"], false);
    assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
    // secret empty string -> omitted in output.
    assert!(cfg.get("secret").is_none());
}

#[test]
fn apply_mihomo_panel_features_disabled_leaves_config_untouched() {
    let yaml =
        "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    let disabled = PanelFeatures {
        tun_enabled: false,
        tun_stack: String::new(),
        clash_api_enabled: false,
        clash_api_secret: String::new(),
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, CoreType::Mihomo, &disabled);

    assert!(cfg.get("tun").is_none());
    assert!(cfg.get("external-controller").is_none());
    assert!(cfg.get("secret").is_none());
    assert!(cfg.get("external-ui").is_none());
    assert!(cfg.get("external-ui-url").is_none());
}

// ---------- Rule mode (mihomo top-level mode injection / sing-box not written) ----------

#[test]
fn apply_mihomo_panel_features_injects_rule_mode() {
    let yaml =
        "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    let features = PanelFeatures {
        rule_mode: "global".to_string(),
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, CoreType::Mihomo, &features);

    assert_eq!(
        cfg["mode"], "global",
        "mihomo top-level should write persisted rule mode"
    );
}

/// Invalid values (including empty string) normalize back to `rule` before writing.
#[test]
fn apply_mihomo_panel_features_falls_back_to_rule_for_invalid_mode() {
    let yaml =
        "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    for invalid in ["", "bogus", "Rule", "direct2"] {
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        let features = PanelFeatures {
            rule_mode: invalid.to_string(),
            ..singbox_features()
        };
        apply_panel_features(&mut cfg, CoreType::Mihomo, &features);
        assert_eq!(
            cfg["mode"], "rule",
            "invalid value {invalid:?} should fall back to rule"
        );
    }
}

/// Template/override already has `mode` -> replaced by settings.
#[test]
fn apply_mihomo_panel_features_mode_overrides_template() {
    let yaml = "mode: global\nmixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    assert_eq!(
        cfg["mode"], "global",
        "template's own mode should be preserved before injection"
    );
    let features = PanelFeatures {
        rule_mode: "direct".to_string(),
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, CoreType::Mihomo, &features);

    assert_eq!(
        cfg["mode"], "direct",
        "settings value should override template mode"
    );
}

/// sing-box has no composition-level mode field: even if rule mode is set, not written to config (runtime
/// switched via Clash API `PATCH /configs`).
#[test]
fn apply_singbox_panel_features_does_not_inject_mode() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        rule_mode: "global".to_string(),
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, CoreType::SingBox, &features);

    assert!(
        cfg.get("mode").is_none(),
        "sing-box config should not write top-level mode: {cfg}"
    );
}

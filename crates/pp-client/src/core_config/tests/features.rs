use super::*;
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
        ipv6_enabled: false,
        dns_fakeip_enabled: false,
        dns_mode: crate::config_slices::DnsMode::FollowSystem,
    }
}

#[test]
fn apply_singbox_panel_features_injects_tun_and_clash_api() {
    let sub = json!({
        "inbounds": [{ "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 17890 }],
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &singbox_features());

    // tun inbound appended: tag / address / mtu / auto_route / stack.
    let inbounds = cfg["inbounds"].as_array().unwrap();
    let tun = inbounds
        .iter()
        .find(|i| i["type"] == "tun")
        .expect("should inject tun inbound");
    assert_eq!(tun["tag"], "tun-in");
    assert_eq!(
        tun["address"],
        json!(["172.19.0.1/30", "fdfe:dcba:9876::1/126"]),
        "tun must be dual-stack to avoid IPv6 blackhole/leak"
    );
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
    apply_panel_features(&mut cfg, &singbox_features());

    let inbounds = cfg["inbounds"].as_array().unwrap();
    let tun: Vec<_> = inbounds.iter().filter(|i| i["type"] == "tun").collect();
    assert_eq!(
        tun.len(),
        1,
        "template tun replaced, only one tun inbound kept"
    );
    assert_eq!(
        tun[0]["address"],
        json!(["172.19.0.1/30", "fdfe:dcba:9876::1/126"])
    );
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
    assert_eq!(
        android_tun["address"],
        json!(["172.19.0.1/30", "fdfe:dcba:9876::1/126"]),
        "Android (libbox) tun must also be dual-stack"
    );
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
    apply_panel_features(&mut cfg, &clash_only);
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

    // local first (DoH, no detour = direct by default), remote uses actual selector tag.
    assert_eq!(cfg["dns"]["servers"][0]["tag"], "local");
    assert_eq!(cfg["dns"]["servers"][0]["type"], "https");
    assert!(cfg["dns"]["servers"][0].get("detour").is_none());
    assert_eq!(cfg["dns"]["servers"][1]["tag"], "remote");
    assert_eq!(cfg["dns"]["servers"][1]["type"], "tls");
    assert_eq!(cfg["dns"]["servers"][1]["detour"], "proxy");
    assert_eq!(
        cfg["dns"]["rules"],
        json!([
            { "clash_mode": "direct", "action": "route", "server": "local" },
            { "clash_mode": "global", "action": "route", "server": "remote" },
            { "rule_set": ["geosite-cn"], "action": "route", "server": "local" }
        ]),
        "Android DNS injection reproduces the CN-split baseline rules"
    );
    assert_eq!(cfg["dns"]["final"], "remote");
    assert_eq!(
        cfg["dns"]["reverse_mapping"], true,
        "reverse mapping must stay on so hijacked-DNS IPs map back to domains for connection records"
    );
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
    // sing-box 1.12+ requires explicit default_domain_resolver (pointing to first tagged server).
    assert_eq!(
        cfg["route"]["default_domain_resolver"],
        json!({ "server": "local" })
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

    assert_eq!(cfg["dns"]["servers"][0]["tag"], "local");
    assert_eq!(cfg["dns"]["servers"][1]["tag"], "remote");
    assert!(
        cfg["dns"]["servers"][1].get("detour").is_none(),
        "remote should omit detour when route.final points to empty direct outbound"
    );
    assert_eq!(
        cfg["route"]["default_domain_resolver"],
        json!({ "server": "local" })
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
    assert_eq!(cfg["dns"]["servers"][0]["tag"], "local");
    assert!(cfg["dns"]["servers"][0].get("detour").is_none());
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
    assert_eq!(cfg["dns"]["servers"][0]["tag"], "local");
    assert!(cfg["dns"]["servers"][0].get("detour").is_none());
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

    assert_eq!(cfg["dns"]["servers"][1]["tag"], "remote");
    assert_eq!(
        cfg["dns"]["servers"][1]["detour"], "direct",
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
    apply_panel_features(&mut cfg, &singbox_features());
    inject_android_dns(&mut cfg);

    // Composed config main selector tag is `proxy` (singbox_template fixed group name).
    assert_eq!(cfg["dns"]["servers"][1]["detour"], "proxy");

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
    apply_panel_features(&mut cfg, &singbox_features());
    inject_android_dns(&mut cfg);

    // No direct outbound -> local DNS has no detour, and outbounds contains no created direct outbound.
    assert_eq!(cfg["dns"]["servers"][0]["tag"], "local");
    assert!(cfg["dns"]["servers"][0].get("detour").is_none());
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

/// 全部面板特性关闭时：tun inbound / experimental.clash_api / clash_mode 规则都不注入，
/// 但 sniff + hijack-dns 头部规则仍无条件注入（TUN 必需，mixed-only 无害）。
#[test]
fn apply_singbox_panel_features_disabled_only_injects_sniff_and_dns_hijack() {
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
    apply_panel_features(&mut cfg, &disabled);

    assert!(
        !cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["type"] == "tun")
    );
    assert!(cfg.get("experimental").is_none());
    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(rules[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
    assert_eq!(
        rules.len(),
        2,
        "clash_api disabled -> no clash_mode rules, only sniff/hijack-dns injected"
    );
}

/// sniff / hijack-dns 幂等：route.rules 已含 sniff 或 hijack-dns 规则时不重复注入（各自独立判断）。
/// 关闭 clash_api（无 mode 规则注入），隔离验证 sniff/hijack 头部插入的幂等语义。
#[test]
fn inject_dns_hijack_and_sniff_skips_when_already_present() {
    let no_clash_api = PanelFeatures {
        clash_api_enabled: false,
        ..singbox_features()
    };

    // 已含 sniff、缺 hijack-dns：只补 hijack-dns（紧随已有 sniff 之后，index 1）。
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": {
            "final": "direct",
            "rules": [
                { "action": "sniff" },
                { "domain": "sub.com", "outbound": "proxy" }
            ]
        }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &no_clash_api);
    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.len(),
        3,
        "missing hijack-dns added next to existing sniff, subscription rule kept"
    );
    assert_eq!(rules[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
    assert_eq!(
        rules[2],
        json!({ "domain": "sub.com", "outbound": "proxy" })
    );

    // 两条都已含 -> 一条都不补，订阅规则原序保留。
    let sub2 = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": {
            "final": "direct",
            "rules": [
                { "action": "sniff" },
                { "protocol": "dns", "action": "hijack-dns" },
                { "domain": "sub.com", "outbound": "proxy" }
            ]
        }
    });
    let mut cfg2 = compose_singbox_config(&sub2, 17890, None).unwrap();
    apply_panel_features(&mut cfg2, &no_clash_api);
    let rules2 = cfg2["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules2.len(),
        3,
        "existing sniff + hijack-dns -> no head injection, rules untouched"
    );
    assert_eq!(rules2[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules2[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
    assert_eq!(
        rules2[2],
        json!({ "domain": "sub.com", "outbound": "proxy" })
    );
}

// ---------- Outbound mode: baseline clash_mode rules + clash_api default_mode ----------

/// `apply_panel_features` (Clash API enabled) injects sniff + hijack-dns head rules and the two
/// baseline `clash_mode` rules into `route.rules` — before subscription rules, local override
/// rules and the MITM whitelist rule (all of which run in earlier stages), so DNS hijacking /
/// sniffing and the mode switch take priority. Final head order:
/// `sniff`, `hijack-dns`, `clash_mode direct`, `clash_mode global`, then the original rule.
#[test]
fn apply_singbox_panel_features_injects_mode_baseline_rules_at_head() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": {
            "final": "direct",
            "rules": [
                { "domain": "sub.com", "outbound": "proxy" }
            ]
        }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &singbox_features());

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.len(),
        5,
        "2 sniff/dns-hijack + 2 baseline mode rules prepended to 1 subscription rule"
    );
    // Head rules: sniff + hijack-dns (TUN DNS hijack, precedes mode rules so DNS goes to DNS module in every mode).
    assert_eq!(rules[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
    // Then the two mode switch baselines (small-case, matching push / mode-list values).
    assert_eq!(
        rules[2],
        json!({ "clash_mode": "direct", "outbound": "direct" })
    );
    assert_eq!(
        rules[3],
        json!({ "clash_mode": "global", "outbound": "proxy" })
    );
    // Original subscription rule preserved after them (mode wins over it).
    assert_eq!(
        rules[4],
        json!({ "domain": "sub.com", "outbound": "proxy" })
    );
}

/// Full route rule order (the layered injection contract): `sniff` / `hijack-dns` / `clash_mode`
/// baselines are prepended ahead of the user (local-override / MITM) rules, which in turn precede
/// the template's CN-split baseline (injected last so user rules always win):
/// `sniff → hijack-dns → clash_mode direct → clash_mode global → user → CN baseline → final`.
#[test]
fn apply_singbox_panel_features_keeps_cn_baseline_after_user_rules() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": {
            "final": "proxy",
            "rules": [
                // user rule already prepended by the local-override / MITM stages
                { "domain": "user.example", "outbound": "proxy" },
                // template CN-split baseline (already appended before panel features)
                { "rule_set": ["geosite-private"], "outbound": "direct" },
                { "rule_set": ["geosite-cn"], "outbound": "direct" },
                { "rule_set": ["geoip-private"], "outbound": "direct" },
                { "rule_set": ["geoip-cn"], "outbound": "direct" },
                { "rule_set": ["geolocation-!cn"], "outbound": "proxy" }
            ]
        }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &singbox_features());

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.len(),
        10,
        "2 sniff/hijack + 2 clash_mode + 1 user + 5 CN baseline"
    );
    assert_eq!(rules[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
    assert_eq!(
        rules[2],
        json!({ "clash_mode": "direct", "outbound": "direct" })
    );
    assert_eq!(
        rules[3],
        json!({ "clash_mode": "global", "outbound": "proxy" })
    );
    assert_eq!(
        rules[4],
        json!({ "domain": "user.example", "outbound": "proxy" }),
        "user rule wins over the CN-split baseline"
    );
    assert_eq!(
        rules[5],
        json!({ "rule_set": ["geosite-private"], "outbound": "direct" })
    );
    assert_eq!(
        rules[9],
        json!({ "rule_set": ["geolocation-!cn"], "outbound": "proxy" }),
        "non-CN baseline is last before route.final"
    );
}

/// `experimental.clash_api.default_mode` is written (normalized small-case) from `features.rule_mode`
/// — core starts in the persisted mode without relying on the post-start Clash API push.
#[test]
fn apply_singbox_panel_features_writes_clash_api_default_mode() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    for (rule_mode, expected) in [("rule", "rule"), ("global", "global"), ("direct", "direct")] {
        let features = PanelFeatures {
            rule_mode: rule_mode.to_string(),
            ..singbox_features()
        };
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut cfg, &features);
        assert_eq!(
            cfg["experimental"]["clash_api"]["default_mode"], expected,
            "default_mode should mirror rule_mode={rule_mode}"
        );
    }
}

/// Invalid `rule_mode` falls back to `rule` at injection time (same normalization as the push path).
#[test]
fn apply_singbox_panel_features_default_mode_falls_back_for_invalid_rule_mode() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let features = PanelFeatures {
        rule_mode: "turbo".to_string(),
        ..singbox_features()
    };
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &features);
    assert_eq!(cfg["experimental"]["clash_api"]["default_mode"], "rule");
}

/// When `route.rules` already carries a `clash_mode` rule (user explicitly took over mode semantics
/// via Profile override / template), the baseline injection is skipped — no duplication/conflict.
/// sniff / hijack-dns 仍注入（与用户接管模式语义无关，无条件）。
#[test]
fn apply_singbox_panel_features_skips_mode_rules_when_clash_mode_already_present() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": {
            "rules": [
                { "clash_mode": "Rule", "outbound": "direct" },
                { "domain": "sub.com", "outbound": "proxy" }
            ]
        }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &singbox_features());

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.len(),
        4,
        "user clash_mode rule present -> baseline not injected, sniff/hijack-dns still injected"
    );
    assert_eq!(rules[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
    assert_eq!(
        rules[2],
        json!({ "clash_mode": "Rule", "outbound": "direct" })
    );
    assert_eq!(
        rules[3],
        json!({ "domain": "sub.com", "outbound": "proxy" })
    );
}

/// Clash API disabled: no `experimental.clash_api` (hence no `default_mode`), no baseline
/// `clash_mode` rules injected (mode subsystem is off, rules would be dead weight) — but sniff /
/// hijack-dns head rules still injected unconditionally.
#[test]
fn apply_singbox_panel_features_no_mode_rules_nor_default_mode_without_clash_api() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": { "final": "direct", "rules": [] }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let no_clash_api = PanelFeatures {
        clash_api_enabled: false,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &no_clash_api);

    assert!(
        cfg.get("experimental").is_none(),
        "clash_api disabled -> no experimental section, no default_mode"
    );
    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.len(),
        2,
        "clash_api disabled -> no clash_mode rules, only sniff/hijack-dns injected"
    );
    assert_eq!(rules[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
}

/// DNS mode derivation (ADR-0005 D1): the slice has no master switch, so the
/// derived mode is simply the configured `dns.mode`, shared by desktop and
/// Android.
#[test]
fn dns_mode_from_slices_truth_table() {
    use crate::config_slices::{ConfigSlices, DnsMode};

    let cases = [
        (DnsMode::FollowSystem, DnsMode::FollowSystem),
        (DnsMode::Takeover, DnsMode::Takeover),
    ];
    for (mode, expected) in cases {
        let mut slices = ConfigSlices::default();
        slices.dns.mode = mode;
        assert_eq!(dns_mode_from_slices(&slices), expected, "mode={mode:?}");
    }
}

// ---------- IPv6 switch: default off rewrites dns.strategy to ipv4_only ----------

/// Default (`ipv6_enabled = false`) desktop template path: the template's `dns.strategy =
/// prefer_ipv4` is rewritten to `ipv4_only`, suppressing AAAA lookups for nodes without an IPv6
/// egress; the rest of the `dns` object is preserved.
#[test]
fn apply_singbox_panel_features_disables_ipv6_by_rewriting_dns_strategy() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }],
            "final": "local",
            "reverse_mapping": true,
            "strategy": "prefer_ipv4"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        ipv6_enabled: false,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    assert_eq!(cfg["dns"]["strategy"], "ipv4_only");
    assert_eq!(cfg["dns"]["final"], "local");
    assert_eq!(cfg["dns"]["reverse_mapping"], true);
}

/// Android injection path ordering: `inject_android_dns` writes `strategy = prefer_ipv4`, the
/// IPv6 override runs after it and wins, so the final strategy is `ipv4_only`.
///
/// The host test cannot exercise the `cfg(target_os = "android")` branch inside
/// `apply_panel_features`, so the injection is applied first explicitly to reproduce the exact
/// production order.
#[test]
fn apply_singbox_panel_features_ipv6_override_wins_over_android_dns_injection() {
    let sub = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["direct"], "default": "direct" },
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "proxy" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    inject_android_dns(&mut cfg);
    assert_eq!(
        cfg["dns"]["strategy"], "prefer_ipv4",
        "android injection baseline"
    );

    let features = PanelFeatures {
        ipv6_enabled: false,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);
    assert_eq!(
        cfg["dns"]["strategy"], "ipv4_only",
        "IPv6 override must run after inject_android_dns and win"
    );
}

/// `ipv6_enabled = true`: template / injected `prefer_ipv4` is left untouched.
#[test]
fn apply_singbox_panel_features_keeps_dns_strategy_when_ipv6_enabled() {
    let sub = json!({
        "dns": { "servers": [], "strategy": "prefer_ipv4" },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        ipv6_enabled: true,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
}

/// DNS slice takeover exempts the IPv6 switch: the user's own `dns.strategy` is preserved even
/// with `ipv6_enabled = false` (user takes over DNS completely, ADR-0005 D1).
#[test]
fn apply_singbox_panel_features_ipv6_override_exempt_on_takeover() {
    let sub = json!({
        "dns": { "servers": [], "strategy": "prefer_ipv6" },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        ipv6_enabled: false,
        dns_mode: crate::config_slices::DnsMode::Takeover,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv6");
}

/// No `dns` object in the composed config -> the IPv6 override does not fabricate one.
#[test]
fn apply_singbox_panel_features_ipv6_override_requires_existing_dns() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    assert!(cfg.get("dns").is_none(), "baseline has no dns object");
    let features = PanelFeatures {
        ipv6_enabled: false,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);
    assert!(cfg.get("dns").is_none());
}

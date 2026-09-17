//! `ClientConfig` 单元测试（从 mod.rs 拆出：业务文件 500 行门禁）。

use super::*;

#[test]
fn default_config_has_sane_defaults() {
    let cfg = ClientConfig::default();
    assert_eq!(cfg.mixed_port, 17890);
    assert!(cfg.mitm_enabled);
    assert!(!cfg.system_proxy_enabled);
    assert!(cfg.mitm.hostnames.is_empty());
    assert!(cfg.active_subscription_id.is_none(), "默认不选中订阅");
    assert!(matches!(
        cfg.mitm.script_dialect,
        pp_script::ScriptDialect::Surge
    ));
    // TUN 默认关闭；Clash API 默认开启（流量统计与出站模式即时切换依赖它，
    // 用户可在设置页显式关闭并持久化）。
    assert!(!cfg.tun_enabled);
    assert_eq!(cfg.tun_stack, "mixed");
    assert!(cfg.tun_auto_route);
    // IPv6 默认关闭：DNS 策略 ipv4_only。
    assert!(!cfg.ipv6_enabled);
    // FakeIP 默认关闭（opt-in）。
    assert!(!cfg.dns_fakeip_enabled);
    assert!(cfg.clash_api_enabled);
    assert_eq!(cfg.clash_api_port, 9090);
    assert!(cfg.clash_api_secret.is_empty());
    assert_eq!(cfg.clash_api_ui, "zashboard");
    // GitHub 访问默认直连：无代理前缀、不走本地代理。
    assert!(cfg.github_proxy_prefix.is_empty());
    assert!(!cfg.fetch_via_local_proxy);
    // 规则模式默认 rule。
    assert_eq!(cfg.rule_mode, "rule");
    assert_eq!(cfg.normalized_rule_mode(), "rule");
    // VPN notification defaults.
    assert!(cfg.vpn_notify_show_traffic);
    assert!(cfg.vpn_notify_show_selection);
}

#[test]
fn migrated_tun_stack_only_rewrites_android_gvisor() {
    assert_eq!(migrated_tun_stack(true, "gvisor"), Some("go"));
    // 其余取值与 desktop 平台一律不动。
    assert_eq!(migrated_tun_stack(true, "go"), None);
    assert_eq!(migrated_tun_stack(true, "mixed"), None);
    assert_eq!(migrated_tun_stack(true, "system"), None);
    assert_eq!(migrated_tun_stack(false, "gvisor"), None);
}

#[test]
fn new_config_wires_ca_dir_to_data_dir() {
    let cfg = ClientConfig::new(
        PathBuf::from("/tmp/pp-client-test"),
        "http://127.0.0.1:50052",
        "abc123",
        PathBuf::from("/usr/local/bin/sing-box"),
    );
    assert_eq!(cfg.mitm.ca_dir, PathBuf::from("/tmp/pp-client-test/certs"));
}

#[test]
fn serde_roundtrip() {
    let mut cfg = ClientConfig::new(
        PathBuf::from("/tmp/pp-client-test"),
        "http://127.0.0.1:50052",
        "abc123",
        PathBuf::from("/usr/local/bin/sing-box"),
    );
    cfg.tun_enabled = true;
    cfg.tun_stack = "system".to_string();
    cfg.tun_auto_route = false;
    cfg.ipv6_enabled = true;
    cfg.dns_fakeip_enabled = true;
    cfg.clash_api_enabled = true;
    cfg.clash_api_port = 9091;
    cfg.clash_api_secret = "sekret".to_string();
    cfg.clash_api_ui = "metacubexd".to_string();
    cfg.github_proxy_prefix = "https://gh-proxy.com".to_string();
    cfg.fetch_via_local_proxy = true;
    cfg.rule_mode = "global".to_string();
    cfg.active_subscription_id = Some(uuid::Uuid::new_v4());
    let json = serde_json::to_string(&cfg).unwrap();
    let back: ClientConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(cfg, back);
}

#[test]
fn serde_missing_new_fields_defaults() {
    // 旧版 client.json 缺失 TUN / Clash 字段时按默认值解析（serde default 全兼容；
    // Clash API 默认开启，同 ClientConfig::default）。
    let json = r#"{
        "data_dir": "/tmp/pp-client-test",
        "hub_url": "http://127.0.0.1:50052",
        "sub_token": "tok",
        "core_type": "singbox",
        "core_binary": "/usr/local/bin/sing-box",
        "mixed_port": 17890,
        "mitm_enabled": true,
        "mitm": { "ca_dir": "/tmp/pp-client-test/certs", "hostnames": [], "script_dialect": "Surge" },
        "system_proxy_enabled": false
    }"#;
    let cfg: ClientConfig = serde_json::from_str(json).unwrap();
    assert!(!cfg.tun_enabled);
    assert_eq!(cfg.tun_stack, "mixed");
    assert!(cfg.tun_auto_route);
    // Old client.json missing ipv6_enabled should parse with default false.
    assert!(!cfg.ipv6_enabled);
    // Old client.json missing dns_fakeip_enabled should parse with default false.
    assert!(!cfg.dns_fakeip_enabled);
    assert!(cfg.clash_api_enabled);
    assert_eq!(cfg.clash_api_port, 9090);
    assert!(cfg.clash_api_secret.is_empty());
    assert_eq!(cfg.clash_api_ui, "zashboard");
    // 旧 client.json 缺失 GitHub 访问字段时按默认值解析（直连、不代理）。
    assert!(cfg.github_proxy_prefix.is_empty());
    assert!(!cfg.fetch_via_local_proxy);
    // 旧 client.json 缺失 active_subscription_id 时按默认值解析（未选中订阅）。
    assert_eq!(cfg.active_subscription_id, None);
    // Old client.json missing group_selections should parse with default empty map.
    assert!(cfg.group_selections.is_empty());
    // Old client.json missing rule_mode should parse with default `rule`.
    assert_eq!(cfg.rule_mode, "rule");
    assert_eq!(cfg.normalized_rule_mode(), "rule");
    // Old client.json missing VPN notification fields should parse with default true.
    assert!(cfg.vpn_notify_show_traffic);
    assert!(cfg.vpn_notify_show_selection);
}

#[test]
fn normalized_rule_mode_falls_back_for_invalid_values() {
    let cfg = ClientConfig {
        rule_mode: "direct".to_string(),
        ..ClientConfig::default()
    };
    assert_eq!(cfg.normalized_rule_mode(), "direct");

    for invalid in ["", "bogus", "Rule", "全局", "proxy"] {
        let cfg = ClientConfig {
            rule_mode: invalid.to_string(),
            ..ClientConfig::default()
        };
        assert_eq!(
            cfg.normalized_rule_mode(),
            "rule",
            "非法值 {invalid:?} 应回退 rule"
        );
    }
}

#[test]
fn load_save_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        "http://127.0.0.1:50052",
        "tok",
        PathBuf::from("/usr/local/bin/sing-box"),
    );
    cfg.hub_url = "http://localhost:50052".to_string();
    cfg.mixed_port = 20000;
    cfg.system_proxy_enabled = true;

    cfg.save().unwrap();
    assert!(dir.path().join("client.json").exists());

    let loaded = ClientConfig::load(dir.path()).unwrap();
    assert_eq!(cfg, loaded);
}

/// 存量归一化：旧版 `core_type: "mihomo"` 的 client.json 加载时归一化——`core_type`
/// 字段被忽略；`core_binary` 指向 mihomo 二进制时重置为空并写回磁盘。
#[test]
fn load_normalizes_legacy_mihomo_config_and_writes_back() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("client.json"),
        r#"{
            "data_dir": "",
            "core_type": "mihomo",
            "core_binary": "/home/u/.proxy-panel-client/cores/mihomo/1.19.29/mihomo",
            "mixed_port": 17890
        }"#,
    )
    .unwrap();

    let loaded = ClientConfig::load(dir.path()).unwrap();
    assert!(
        loaded.core_binary.as_os_str().is_empty(),
        "mihomo 二进制应重置为自动选择"
    );

    // 已写回：再次读取磁盘文件，core_type 字段被剔除、core_binary 为空。
    let text = std::fs::read_to_string(dir.path().join("client.json")).unwrap();
    let raw: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(raw.get("core_type").is_none());
    assert_eq!(raw["core_binary"], "");

    // 再次加载幂等（不报错）。
    let again = ClientConfig::load(dir.path()).unwrap();
    assert_eq!(again, loaded);
}

/// 存量归一化：旧版 `core_type: "mihomo"` 但 core_binary 指向非 mihomo 路径时，
/// 保留 core_binary 原值。
#[test]
fn load_normalizes_legacy_mihomo_keeps_non_mihomo_binary() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("client.json"),
        r#"{
            "data_dir": "",
            "core_type": "mihomo",
            "core_binary": "/usr/local/bin/sing-box"
        }"#,
    )
    .unwrap();

    let loaded = ClientConfig::load(dir.path()).unwrap();
    assert_eq!(loaded.core_binary, PathBuf::from("/usr/local/bin/sing-box"));
}

/// 旧版 `core_type: "singbox"` 配置正常加载，字段被忽略、core_binary 不受影响。
#[test]
fn load_tolerates_legacy_singbox_core_type_field() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("client.json"),
        r#"{
            "data_dir": "",
            "core_type": "singbox",
            "core_binary": "/usr/local/bin/sing-box"
        }"#,
    )
    .unwrap();

    let loaded = ClientConfig::load(dir.path()).unwrap();
    assert_eq!(loaded.core_binary, PathBuf::from("/usr/local/bin/sing-box"));
    // 非 mihomo 存量不回写（core_type 字段在下次 save 时自然剔除）。
    let text = std::fs::read_to_string(dir.path().join("client.json")).unwrap();
    assert!(text.contains("core_type"));
}

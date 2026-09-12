//! Profile module tests.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::StatusCode;
use pp_common::PanelError;
use pp_script::{HttpExecutor, HttpRequestSpec};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::core_config::{MitmChain, compose_singbox_config};
use crate::profile::{
    DenyHttpExecutor, EffectiveOverrides, Profile, ProfileOverrides, ProfileStore, ProfileStoreV2,
    apply_js_override, apply_yaml_override, build_core_config, build_core_config_v2,
    extract_nodes_singbox, resolve_remote_overrides, singbox_template,
};

fn sample_singbox_sub() -> Value {
    json!({
        "outbounds": [
            { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
              "uuid": "12345678-1234-1234-1234-123456789012",
              "tls": { "enabled": true, "server_name": "example.com" } },
            { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
              "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
            { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
            { "type": "urltest", "tag": "auto", "outbounds": ["n1"] },
            { "type": "direct", "tag": "direct" },
            { "type": "block", "tag": "block" }
        ]
    })
}

fn mitm_chain() -> MitmChain {
    MitmChain {
        proxy_addr: "127.0.0.1:34567".parse().unwrap(),
        return_port: 17891,
        hostnames: vec!["*.example.com".to_string()],
    }
}

/// Real core binary directory: `target/test-cores` (under workspace root). Tests that need
/// real cores skip directly when missing.
fn test_core_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cores")
}

fn sing_box_binary() -> Option<std::path::PathBuf> {
    let p = test_core_dir().join("sing-box");
    p.is_file().then_some(p)
}

// ---------- ① Node extraction (sing-box) ----------

#[test]
fn extract_nodes_singbox_keeps_leaves_dedups_tags() {
    let nodes = extract_nodes_singbox(&sample_singbox_sub());
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["tag"], "n1");
    assert_eq!(nodes[1]["tag"], "n2");

    // Duplicate tag deduplication: -2 / -3.
    let sub = json!({
        "outbounds": [
            { "type": "vless", "tag": "dup", "server": "a.com", "server_port": 443 },
            { "type": "vmess", "tag": "dup", "server": "b.com", "server_port": 443 },
            { "type": "trojan", "tag": "dup", "server": "c.com", "server_port": 443 },
            { "type": "selector", "tag": "proxy" }
        ]
    });
    let nodes = extract_nodes_singbox(&sub);
    let tags: Vec<&str> = nodes.iter().map(|n| n["tag"].as_str().unwrap()).collect();
    assert_eq!(tags, vec!["dup", "dup-2", "dup-3"]);

    // Leaf nodes missing tag are skipped.
    let sub = json!({
        "outbounds": [
            { "type": "vless", "server": "a.com", "server_port": 443 },
            { "type": "vless", "tag": "ok", "server": "b.com", "server_port": 443 }
        ]
    });
    let nodes = extract_nodes_singbox(&sub);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["tag"], "ok");
}

// ---------- ② sing-box template ----------

#[test]
fn singbox_template_builds_groups_and_route() {
    let nodes = extract_nodes_singbox(&sample_singbox_sub());
    let cfg = singbox_template(&nodes);

    let outbounds = cfg["outbounds"].as_array().unwrap();
    // All leaf nodes preserved.
    assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
    assert!(outbounds.iter().any(|o| o["tag"] == "n2"));

    let proxy = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
    assert_eq!(proxy["type"], "selector");
    assert_eq!(proxy["default"], "auto");
    let proxy_out: Vec<&str> = proxy["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(proxy_out, vec!["auto", "n1", "n2"]);

    let auto = outbounds.iter().find(|o| o["tag"] == "auto").unwrap();
    assert_eq!(auto["type"], "urltest");
    assert_eq!(auto["url"], "https://www.gstatic.com/generate_204");
    assert_eq!(auto["interval"], "5m");
    let auto_out: Vec<&str> = auto["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(auto_out, vec!["n1", "n2"]);

    assert!(
        outbounds
            .iter()
            .any(|o| o["tag"] == "direct" && o["type"] == "direct")
    );
    assert!(
        outbounds
            .iter()
            .any(|o| o["tag"] == "block" && o["type"] == "block")
    );

    assert_eq!(cfg["route"]["final"], "proxy");
    assert_eq!(
        cfg["route"]["rules"],
        json!([
            { "rule_set": ["geosite-private"], "outbound": "direct" },
            { "rule_set": ["geosite-cn"], "outbound": "direct" },
            { "rule_set": ["geoip-private"], "outbound": "direct" },
            { "rule_set": ["geoip-cn"], "outbound": "direct" },
            { "rule_set": ["geolocation-!cn"], "outbound": "proxy" }
        ]),
        "CN-split baseline route rules (GUI.for.SingBox default profile)"
    );
    assert_eq!(cfg["route"]["auto_detect_interface"], true);
    assert_eq!(
        cfg["route"]["default_domain_resolver"],
        json!({ "server": "local" })
    );
    assert_eq!(cfg["log"]["level"], "info");

    // DNS: local DoH direct (IP literal, no detour), remote DoH through proxy,
    // final pinned to remote, reverse mapping on (see singbox_template docs).
    let dns_servers = cfg["dns"]["servers"].as_array().unwrap();
    let local = dns_servers.iter().find(|s| s["tag"] == "local").unwrap();
    assert_eq!(local["type"], "https");
    assert_eq!(local["server"], "223.5.5.5");
    assert_eq!(local["server_port"], 443);
    assert!(
        local.get("detour").is_none(),
        "local must stay detour-less (default direct dial)"
    );
    let remote = dns_servers.iter().find(|s| s["tag"] == "remote").unwrap();
    assert_eq!(
        remote["type"], "https",
        "remote resolver is DoH on 443 (not DoT on 853), sharing the HTTPS path through the proxy"
    );
    assert_eq!(remote["server"], "8.8.8.8");
    assert_eq!(remote["server_port"], 443);
    assert_eq!(
        remote["detour"], "proxy",
        "remote DNS must be dialed through the main selector to avoid pollution"
    );
    assert_eq!(
        cfg["dns"]["rules"],
        json!([
            { "clash_mode": "direct", "action": "route", "server": "local" },
            { "clash_mode": "global", "action": "route", "server": "remote" },
            { "rule_set": ["geosite-cn"], "action": "route", "server": "local" }
        ]),
        "CN-split baseline DNS rules (GUI.for.SingBox default profile)"
    );
    assert_eq!(
        cfg["dns"]["final"], "remote",
        "final must pin remote; new-format DNS servers do not follow route.final"
    );
    assert_eq!(cfg["dns"]["reverse_mapping"], true);
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");

    // Remote rule sets download through the direct-dial HTTP client (never via the proxy),
    // resolved by the direct `local` DNS (see ensure_cn_rule_sets / ensure_rule_set_http_client).
    let rule_sets = cfg["route"]["rule_set"].as_array().unwrap();
    assert_eq!(rule_sets.len(), 5, "five CN-split remote rule sets");
    for rs in rule_sets {
        assert_eq!(
            rs["http_client"], "rule-set-direct",
            "remote rule set {} must download via the direct HTTP client",
            rs["tag"]
        );
    }
    assert_eq!(
        cfg["http_clients"],
        json!([{ "tag": "rule-set-direct", "domain_resolver": { "server": "local" } }]),
        "top-level http_clients registers the direct-dial rule-set client"
    );
}

// ---------- ④ YAML deep-merge override ----------

#[test]
fn yaml_override_merges_nested_replaces_arrays_adds_keys() {
    let config = json!({
        "route": { "final": "proxy", "rules": [] },
        "dns": { "enable": true, "nameserver": ["1.1.1.1"] },
        "log": { "level": "info" }
    });
    let yaml = r#"
route:
  final: direct
dns:
  nameserver:
    - 223.5.5.5
log:
  level: debug
new-key:
  a: 1
"#;
    let merged = apply_yaml_override(config, yaml).unwrap();
    // Nested objects merge recursively.
    assert_eq!(merged["route"]["final"], "direct");
    assert_eq!(merged["route"]["rules"], json!([]));
    // Arrays are replaced entirely.
    assert_eq!(merged["dns"]["nameserver"], json!(["223.5.5.5"]));
    assert_eq!(merged["dns"]["enable"], true);
    // Scalar replacement + new key.
    assert_eq!(merged["log"]["level"], "debug");
    assert_eq!(merged["new-key"]["a"], 1);
}

#[test]
fn yaml_override_empty_or_null_keeps_config() {
    let config = json!({ "route": { "final": "proxy" } });
    assert_eq!(apply_yaml_override(config.clone(), "").unwrap(), config);
    assert_eq!(apply_yaml_override(config.clone(), "   ").unwrap(), config);
    assert_eq!(apply_yaml_override(config.clone(), "null").unwrap(), config);
    assert!(
        apply_yaml_override(config, "# only a comment\n")
            .unwrap()
            .as_object()
            .is_some()
    );
}

#[test]
fn yaml_override_rejects_non_mapping_and_bad_yaml() {
    let config = json!({ "route": { "final": "proxy" } });
    assert!(matches!(
        apply_yaml_override(config.clone(), "- a\n- b").unwrap_err(),
        PanelError::Client(_)
    ));
    assert!(matches!(
        apply_yaml_override(config, "route: [unclosed").unwrap_err(),
        PanelError::Client(_)
    ));
}

// ---------- ⑤ JS override ----------

#[tokio::test(flavor = "current_thread")]
async fn js_override_mutates_config_and_returns() {
    let config = json!({ "route": { "final": "proxy" }, "log": { "level": "info" } });
    let js = r#"function main(c) { c.route.final = "direct"; return c; }"#;
    let out = apply_js_override(config, js).await.unwrap();
    assert_eq!(out["route"]["final"], "direct");
}

#[tokio::test(flavor = "current_thread")]
async fn js_override_without_return_keeps_config() {
    let config = json!({ "route": { "final": "proxy" } });
    let js = "function main(c) { /* intentionally no return */ }";
    let out = apply_js_override(config, js).await.unwrap();
    assert_eq!(out["route"]["final"], "proxy");
}

#[tokio::test(flavor = "current_thread")]
async fn js_override_empty_source_keeps_config() {
    let config = json!({ "route": { "final": "proxy" } });
    assert_eq!(apply_js_override(config.clone(), "").await.unwrap(), config);
    assert_eq!(
        apply_js_override(config, "  ").await.unwrap()["route"]["final"],
        "proxy"
    );
}

/// Deny environment: under Surge dialect `$task` is not injected (undefined, type-level);
/// `$httpClient` exists but its network is denied by [`DenyHttpExecutor`] (see
/// `deny_http_executor_always_denies`).
#[tokio::test(flavor = "current_thread")]
async fn js_override_task_undefined_and_network_denied() {
    let config = json!({ "log": { "level": "info" } });
    let js = r#"
            function main(c) {
                c.taskType = typeof $task;
                c.httpClientType = typeof $httpClient;
                return c;
            }
        "#;
    let out = apply_js_override(config, js).await.unwrap();
    assert_eq!(out["taskType"], "undefined");
    assert_eq!(out["httpClientType"], "object");
}

#[tokio::test(flavor = "current_thread")]
async fn deny_http_executor_always_denies() {
    let err = DenyHttpExecutor
        .execute(HttpRequestSpec {
            url: "http://example.com/".to_string(),
            ..HttpRequestSpec::default()
        })
        .await
        .unwrap_err();
    assert!(matches!(err, PanelError::Client(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn js_override_invalid_script_errors() {
    let config = json!({ "route": { "final": "proxy" } });
    let err = apply_js_override(config, "function main(c) { return c;")
        .await
        .unwrap_err();
    assert!(matches!(err, PanelError::Script(_)));
}

// ---------- ⑥ Send compile-time assertion ----------

/// Compile-time assertion: `build_core_config` future is `Send` (`apply_js_override` driven
/// by [`ScriptWorker`] no longer contains rquickjs non-`Send` structures, can be awaited
/// across threads).
#[test]
fn build_core_config_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    let sub = sample_singbox_sub();
    let overrides = ProfileOverrides::default();
    let fut = build_core_config(&sub, &overrides);
    assert_send(&fut);
}

// ---------- ⑦ End-to-end ----------

#[tokio::test(flavor = "current_thread")]
async fn build_core_config_singbox_end_to_end() {
    let overrides = ProfileOverrides {
        yaml_override: "route:\n  final: direct\n".to_string(),
        js_override: r#"function main(c) { c.log.level = "error"; return c; }"#.to_string(),
    };
    let cfg = build_core_config(&sample_singbox_sub(), &overrides)
        .await
        .unwrap();

    // Nodes present.
    let outbounds = cfg["outbounds"].as_array().unwrap();
    assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
    assert!(outbounds.iter().any(|o| o["tag"] == "n2"));
    // Groups present.
    let proxy = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
    let proxy_list: Vec<&str> = proxy["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(proxy_list.contains(&"n1") && proxy_list.contains(&"n2"));
    // YAML override takes effect.
    assert_eq!(cfg["route"]["final"], "direct");
    // JS override (after YAML) takes effect.
    assert_eq!(cfg["log"]["level"], "error");
}

// ---------- ⑧ Regression: compose_* injection (template baseline rules prepending OK) ----------

#[tokio::test(flavor = "current_thread")]
async fn compose_singbox_injects_inbounds_and_mitm_into_profile_output() {
    let cfg = build_core_config(&sample_singbox_sub(), &ProfileOverrides::default())
        .await
        .unwrap();
    assert_eq!(
        cfg["route"]["rules"],
        json!([
            { "rule_set": ["geosite-private"], "outbound": "direct" },
            { "rule_set": ["geosite-cn"], "outbound": "direct" },
            { "rule_set": ["geoip-private"], "outbound": "direct" },
            { "rule_set": ["geoip-cn"], "outbound": "direct" },
            { "rule_set": ["geolocation-!cn"], "outbound": "proxy" }
        ])
    );

    let composed = compose_singbox_config(&cfg, 17890, Some(mitm_chain())).unwrap();

    // inbounds injection (main entry + return flow).
    let inbounds = composed["inbounds"].as_array().unwrap();
    assert_eq!(inbounds.len(), 2);
    assert_eq!(inbounds[0]["tag"], "main-in");
    assert_eq!(inbounds[1]["tag"], "mitm-return");

    // MITM whitelist rule prepends ahead of the template CN-split baseline.
    let rules = composed["route"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 6);
    assert_eq!(rules[0]["outbound"], "pp-mitm");
    assert_eq!(rules[0]["domain_suffix"], json!(["example.com"]));
    assert_eq!(
        rules[1],
        json!({ "rule_set": ["geosite-private"], "outbound": "direct" }),
        "CN-split baseline stays after the MITM whitelist (user rule wins)"
    );
    assert_eq!(composed["route"]["final"], "proxy");
    // Groups and nodes preserved.
    let outbounds = composed["outbounds"].as_array().unwrap();
    assert!(outbounds.iter().any(|o| o["tag"] == "proxy"));
    assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
}

// ---------- Real core check (when test-cores exists, must verify template field compatibility) ----------

#[test]
fn singbox_template_passes_real_singbox_check() {
    let Some(bin) = sing_box_binary() else {
        return;
    };
    let nodes = extract_nodes_singbox(&sample_singbox_sub());
    let cfg = compose_singbox_config(&singbox_template(&nodes), 17890, Some(mitm_chain())).unwrap();
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
        "sing-box check failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------- ProfileStore ----------

#[test]
fn profile_store_roundtrip_and_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().to_path_buf());

    // Missing → default.
    assert_eq!(store.load().unwrap(), ProfileOverrides::default());

    let overrides = ProfileOverrides {
        yaml_override: "route:\n  final: direct\n".to_string(),
        js_override: "function main(c) { return c; }".to_string(),
    };
    store.save(&overrides).unwrap();
    assert!(store.profile_file().exists());
    assert_eq!(store.load().unwrap(), overrides);
}

#[test]
fn profile_store_tolerates_corrupted_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStore::new(dir.path().to_path_buf());
    std::fs::write(store.profile_file(), "{ not json").unwrap();
    assert_eq!(store.load().unwrap(), ProfileOverrides::default());
}

// ---------- ProfileStoreV2 (multi-template + legacy migration) ----------

#[test]
fn profile_store_v2_loads_empty_when_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStoreV2::new(dir.path().to_path_buf());
    assert_eq!(store.load().unwrap(), Vec::<Profile>::new());
    assert!(!store.profiles_file().exists());
}

/// ① Migration: old profile.json → default Profile (overrides preserved)
/// and old file deleted.
#[test]
fn profile_store_v2_migrates_legacy_profile_json_once() {
    let dir = tempfile::tempdir().unwrap();
    // Pre-place legacy profile.json.
    ProfileStore::new(dir.path().to_path_buf())
        .save(&ProfileOverrides {
            yaml_override: "route:\n  final: direct\n".to_string(),
            js_override: "function main(c) { return c; }".to_string(),
        })
        .unwrap();
    let legacy = dir.path().join("profile.json");
    assert!(legacy.exists());

    let store = ProfileStoreV2::new(dir.path().to_path_buf());
    let profiles = store.load().unwrap();

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "Default");
    assert_eq!(profiles[0].yaml_override, "route:\n  final: direct\n");
    assert_eq!(profiles[0].js_override, "function main(c) { return c; }");

    // One-time migration: old file deleted, second load does not migrate, id kept.
    assert!(!legacy.exists());
    let again = store.load().unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].id, profiles[0].id);
}

#[test]
fn profile_store_v2_migrates_corrupted_legacy_with_empty_overrides() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("profile.json"), "{ not json").unwrap();
    let store = ProfileStoreV2::new(dir.path().to_path_buf());
    let profiles = store.load().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "Default");
    assert_eq!(profiles[0].yaml_override, "");
    assert_eq!(profiles[0].js_override, "");
    assert!(!dir.path().join("profile.json").exists());
}

/// 存量归一化：`profiles.json` 中旧版 `core_type: "mihomo"` 的模板在 load 时剔除并写回；
/// `core_type: "singbox"` 的模板保留（core_type 字段被 serde 忽略）。
#[test]
fn profile_store_v2_load_drops_legacy_mihomo_profiles_and_writes_back() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("profiles.json"),
        r#"[
            {"id":"00000000-0000-0000-0000-000000000001","name":"sb","core_type":"singbox","yaml_override":"","js_override":""},
            {"id":"00000000-0000-0000-0000-000000000002","name":"mh","core_type":"mihomo","yaml_override":"mode: global","js_override":""}
        ]"#,
    )
    .unwrap();

    let store = ProfileStoreV2::new(dir.path().to_path_buf());
    let profiles = store.load().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "sb");

    // 已写回：再次读取磁盘文件只剩 sing-box 模板。
    let text = std::fs::read_to_string(dir.path().join("profiles.json")).unwrap();
    assert!(text.contains("\"sb\""));
    assert!(!text.contains("\"mh\""));

    // 再次加载幂等。
    assert_eq!(store.load().unwrap().len(), 1);
}

/// ② add: new template does not carry enabled state (pure association model); duplicate name
/// errors.
#[test]
fn profile_store_v2_add_creates_profiles_without_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStoreV2::new(dir.path().to_path_buf());

    let a = store.add("A").unwrap();
    assert!(!a.id.is_nil());

    let b = store.add("B").unwrap();
    assert_ne!(a.id, b.id);

    // Duplicate name errors.
    let err = store.add("A").unwrap_err();
    assert!(matches!(err, PanelError::Client(_)));

    // Disk state consistent with memory.
    let loaded = store.load().unwrap();
    assert_eq!(loaded.len(), 2);
}

/// ④ update/remove semantics: update updates name/yaml/js by id, remove deletes by id;
/// both error on non-existent id.
#[test]
fn profile_store_v2_update_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProfileStoreV2::new(dir.path().to_path_buf());
    let mut p = store.add("A").unwrap();

    p.name = "A-renamed".to_string();
    p.yaml_override = "route:\n  final: direct\n".to_string();
    p.js_override = "function main(c) { return c; }".to_string();
    p.yaml_url = Some("https://example.com/r.yaml".to_string());
    p.js_url = Some("https://example.com/r.js".to_string());
    store.update(&p).unwrap();

    let loaded = store.load().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "A-renamed");
    assert_eq!(loaded[0].yaml_override, "route:\n  final: direct\n");
    assert_eq!(loaded[0].js_override, "function main(c) { return c; }");
    assert_eq!(
        loaded[0].yaml_url.as_deref(),
        Some("https://example.com/r.yaml")
    );
    assert_eq!(
        loaded[0].js_url.as_deref(),
        Some("https://example.com/r.js")
    );

    // update on non-existent id errors.
    let ghost = Profile {
        id: Uuid::new_v4(),
        ..p.clone()
    };
    assert!(matches!(
        store.update(&ghost).unwrap_err(),
        PanelError::Client(_)
    ));

    // remove: deletes by id then empty.
    store.remove(p.id).unwrap();
    assert_eq!(store.load().unwrap().len(), 0);

    // remove on non-existent id errors.
    assert!(matches!(
        store.remove(Uuid::new_v4()).unwrap_err(),
        PanelError::Client(_)
    ));
}

// ---------- ⑩ Remote override URL: resolve_remote_overrides + overlay ----------

/// Convenience constructor for Profile (default no remote URL, empty local overrides).
fn remote_test_profile(yaml_url: Option<String>, js_url: Option<String>) -> Profile {
    Profile {
        id: Uuid::new_v4(),
        name: "Remote".to_string(),
        yaml_override: String::new(),
        js_override: String::new(),
        yaml_url,
        js_url,
    }
}

/// Start local server: first request returns `first_body`, subsequent requests always 500
/// (verifies cache fallback).
async fn spawn_toggle_server(first_body: &'static str) -> SocketAddr {
    let hits = Arc::new(AtomicUsize::new(0));
    let app_hits = Arc::clone(&hits);
    let app = axum::Router::new().fallback(move |_req: axum::extract::Request| {
        let app_hits = Arc::clone(&app_hits);
        async move {
            if app_hits.fetch_add(1, Ordering::SeqCst) == 0 {
                (StatusCode::OK, first_body)
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "oops")
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// Start local server: all requests return `body`.
async fn spawn_ok_server(body: &'static str) -> SocketAddr {
    let app = axum::Router::new().fallback(move || async move { (StatusCode::OK, body) });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// Start local server: all requests always 500 (verifies "failure and no cache" path).
async fn spawn_500_server() -> SocketAddr {
    let app = axum::Router::new()
        .fallback(move || async move { (StatusCode::INTERNAL_SERVER_ERROR, "oops") });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// ① Remote YAML + local YAML overlay: remote a=1, b=1; local b=2 → final a=1, b=2.
#[tokio::test(flavor = "current_thread")]
async fn v2_yaml_remote_then_local_overlay() {
    let effective = EffectiveOverrides {
        remote_yaml: "a: 1\nb: 1\n".to_string(),
        local_yaml: "b: 2\n".to_string(),
        ..EffectiveOverrides::default()
    };
    let cfg = build_core_config_v2(
        &sample_singbox_sub(),
        &effective,
        &crate::config_slices::ConfigSlices::default(),
    )
    .await
    .unwrap();
    assert_eq!(cfg["a"], 1, "remote new key should be kept");
    assert_eq!(cfg["b"], 2, "local should override remote's b");
}

/// ② Remote JS + local JS chain: remote main sets x=1, local main sets y=x+1 → local sees
/// remote result.
#[tokio::test(flavor = "current_thread")]
async fn v2_js_remote_then_local_chain() {
    let effective = EffectiveOverrides {
        remote_js: "function main(c) { c.x = 1; return c; }".to_string(),
        local_js: "function main(c) { c.y = c.x + 1; return c; }".to_string(),
        ..EffectiveOverrides::default()
    };
    let cfg = build_core_config_v2(
        &sample_singbox_sub(),
        &effective,
        &crate::config_slices::ConfigSlices::default(),
    )
    .await
    .unwrap();
    assert_eq!(cfg["x"], 1, "remote main should take effect");
    assert_eq!(
        cfg["y"], 2,
        "local main should see remote result (y = x + 1)"
    );
}

/// ③ Fetch failure falls back to cache: first success writes cache (yaml/js), then 500 →
/// uses cached content.
#[tokio::test(flavor = "current_thread")]
async fn resolve_remote_overrides_fetches_writes_cache_and_falls_back() {
    let yaml_body = "route:\n  final: direct\n";
    let js_body = "function main(c) { c.log.level = \"error\"; return c; }";
    let toggle_addr = spawn_toggle_server(yaml_body).await;
    let ok_addr = spawn_ok_server(js_body).await;
    let dir = tempfile::tempdir().unwrap();
    let profile = remote_test_profile(
        Some(format!("http://{toggle_addr}/yaml")),
        Some(format!("http://{ok_addr}/js")),
    );
    // Consistent with production caller structure: pass `data_dir/profile_cache` (shared
    // pipeline derives data_dir from parent).
    let cache = dir.path().join("profile_cache");

    // First time: yaml/js both fetched successfully and cache written, no warnings.
    let (effective, warnings) = resolve_remote_overrides(&cache, &profile).await;
    assert!(warnings.is_empty(), "warnings: {warnings:?}");
    assert_eq!(effective.remote_yaml, yaml_body);
    assert_eq!(effective.remote_js, js_body);
    let yaml_cache = cache.join(format!("{}.yaml", profile.id));
    let js_cache = cache.join(format!("{}.js", profile.id));
    assert_eq!(std::fs::read_to_string(&yaml_cache).unwrap(), yaml_body);
    assert_eq!(std::fs::read_to_string(&js_cache).unwrap(), js_body);

    // Second time: yaml fetch fails (500) falls back to cache; js still succeeds.
    let (effective, warnings) = resolve_remote_overrides(&cache, &profile).await;
    assert_eq!(
        effective.remote_yaml, yaml_body,
        "yaml should fall back to cache"
    );
    assert_eq!(effective.remote_js, js_body, "js should still succeed");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("yaml") && w.contains("fall back to cached")),
        "warnings: {warnings:?}"
    );

    // Full overlay pipeline: remote YAML base + local override + remote JS.
    let effective = EffectiveOverrides {
        local_yaml: "route:\n  final: block\n".to_string(),
        ..effective
    };
    let cfg = build_core_config_v2(
        &sample_singbox_sub(),
        &effective,
        &crate::config_slices::ConfigSlices::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        cfg["route"]["final"], "block",
        "local YAML should override remote"
    );
    assert_eq!(cfg["log"]["level"], "error", "remote JS should take effect");
}

/// ④ Fetch failure and no cache → warning skips that remote override (remote is empty string),
/// no error.
#[tokio::test(flavor = "current_thread")]
async fn resolve_remote_overrides_fetch_failure_without_cache_warns_and_skips() {
    let addr = spawn_500_server().await;
    let dir = tempfile::tempdir().unwrap();
    let profile = remote_test_profile(Some(format!("http://{addr}/yaml")), None);

    let (effective, warnings) =
        resolve_remote_overrides(&dir.path().join("profile_cache"), &profile).await;
    assert_eq!(
        effective.remote_yaml, "",
        "no cache should skip remote override"
    );
    assert_eq!(effective.local_yaml, "");
    assert!(
        warnings.iter().any(|w| w.contains("no cached copy")),
        "warnings: {warnings:?}"
    );
    assert!(
        !dir.path()
            .join("profile_cache")
            .join(format!("{}.yaml", profile.id))
            .exists()
    );
}

/// ⑤ Pure local regression: resolve produces local overrides, v2 and old signature
/// build_core_config are consistent.
#[tokio::test(flavor = "current_thread")]
async fn resolve_remote_overrides_pure_local_regression() {
    let dir = tempfile::tempdir().unwrap();
    let profile = Profile {
        yaml_override: "route:\n  final: direct\n".to_string(),
        js_override: "function main(c) { c.log.level = \"error\"; return c; }".to_string(),
        ..remote_test_profile(None, None)
    };

    let (effective, warnings) = resolve_remote_overrides(dir.path(), &profile).await;
    assert!(warnings.is_empty(), "warnings: {warnings:?}");
    assert_eq!(effective.remote_yaml, "");
    assert_eq!(effective.remote_js, "");
    assert_eq!(effective.local_yaml, profile.yaml_override);
    assert_eq!(effective.local_js, profile.js_override);

    let sub = sample_singbox_sub();
    let legacy = build_core_config(
        &sub,
        &ProfileOverrides {
            yaml_override: profile.yaml_override.clone(),
            js_override: profile.js_override.clone(),
        },
    )
    .await
    .unwrap();
    let v2 = build_core_config_v2(
        &sub,
        &effective,
        &crate::config_slices::ConfigSlices::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        legacy, v2,
        "v2 pure local should be consistent with old signature"
    );
    assert_eq!(v2["route"]["final"], "direct");
    assert_eq!(v2["log"]["level"], "error");
}

/// Compile-time assertion: `build_core_config_v2` / `resolve_remote_overrides` futures are `Send`.
#[test]
fn remote_overrides_futures_are_send() {
    fn assert_send<T: Send>(_: &T) {}
    let sub = sample_singbox_sub();
    let effective = EffectiveOverrides::default();
    let slices = crate::config_slices::ConfigSlices::default();
    let fut = build_core_config_v2(&sub, &effective, &slices);
    assert_send(&fut);
    let profile = remote_test_profile(None, None);
    let fut = resolve_remote_overrides(Path::new("/tmp"), &profile);
    assert_send(&fut);
}

// ---------- ⑪ Config slice layer (⓪, ADR-0005 §3.2) ----------

/// Slice layer is applied after the template and before the YAML/JS overrides:
/// the slice DNS + custom outbound show up in the template output, but a later
/// YAML override still wins on conflicting fields (D2: escape hatch wins).
#[tokio::test(flavor = "current_thread")]
async fn v2_slice_layer_applies_before_yaml_override() {
    use crate::config_slices::ConfigSlices;

    let slices: ConfigSlices = serde_json::from_value(json!({
        "version": 2,
        "dns": {
            "mode": "takeover",
            "servers": [
                { "tag": "slice-dns", "type": "udp", "server": "9.9.9.9", "server_port": 53 }
            ],
            "final_tag": "slice-dns",
            "strategy": "prefer_ipv4"
        },
        "outbounds": {
            "items": [{
                "id": "o1",
                "name": "My Slice",
                "enabled": true,
                "type": "vless",
                "server": "slice.example",
                "server_port": 443,
                "uuid": "12345678-1234-1234-1234-123456789012"
            }]
        }
    }))
    .unwrap();

    // YAML override replaces `dns.final` (conflict) but leaves the slice servers alone.
    let effective = EffectiveOverrides {
        local_yaml: "dns:\n  final: overridden\n".to_string(),
        ..EffectiveOverrides::default()
    };
    let cfg = build_core_config_v2(&sample_singbox_sub(), &effective, &slices)
        .await
        .unwrap();

    // ⓪ slice outbound injected.
    let outbounds = cfg["outbounds"].as_array().unwrap();
    assert!(
        outbounds
            .iter()
            .any(|o| o["tag"] == "slice-my-slice" && o["type"] == "vless"),
        "slice outbound must be injected into the template outbounds"
    );
    // ⓪ slice DNS injected (server present).
    let servers = cfg["dns"]["servers"].as_array().unwrap();
    assert!(
        servers.iter().any(|s| s["tag"] == "slice-dns"),
        "slice DNS server must be injected"
    );
    // ① YAML override wins over the slice on the conflicting key (applied later).
    assert_eq!(
        cfg["dns"]["final"], "overridden",
        "YAML override must win over the slice DNS final (ADR-0005 D2)"
    );
}

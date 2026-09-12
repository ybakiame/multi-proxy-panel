//! CN-split baseline rule-set registration tests for the sing-box template.

use serde_json::json;

use crate::core_config::ensure_cn_rule_sets;
use crate::profile::singbox_template;

/// The template registers the five remote CN-split rule sets with the MetaCubeX jsDelivr URLs.
#[test]
fn singbox_template_registers_cn_rule_sets() {
    let cfg = singbox_template(&[]);
    let rule_sets = cfg["route"]["rule_set"].as_array().unwrap();
    assert_eq!(rule_sets.len(), 5);
    for (tag, url) in [
        (
            "geosite-private",
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/private.srs",
        ),
        (
            "geoip-private",
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geoip/private.srs",
        ),
        (
            "geosite-cn",
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/cn.srs",
        ),
        (
            "geoip-cn",
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geoip/cn.srs",
        ),
        (
            "geolocation-!cn",
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/geolocation-!cn.srs",
        ),
    ] {
        let entry = rule_sets
            .iter()
            .find(|rs| rs["tag"] == tag)
            .unwrap_or_else(|| panic!("missing rule_set {tag}"));
        assert_eq!(entry["type"], "remote");
        assert_eq!(entry["format"], "binary");
        assert_eq!(entry["url"], url);
    }
}

#[test]
fn singbox_template_empty_nodes_falls_back_to_direct() {
    let cfg = singbox_template(&[]);
    let auto = cfg["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["tag"] == "auto")
        .unwrap();
    assert_eq!(auto["outbounds"], json!(["direct"]));
}

/// The CN-split rule-set registry is idempotent by tag: re-running the ensure helper neither
/// duplicates nor overwrites entries.
#[test]
fn cn_rule_set_registration_is_idempotent() {
    let cfg = singbox_template(&[]);
    let mut route = cfg["route"].as_object().unwrap().clone();
    let before = route["rule_set"].clone();
    ensure_cn_rule_sets(&mut route);
    assert_eq!(
        route["rule_set"], before,
        "second registration must be a no-op"
    );

    // A user/override supplied same-tag entry is respected, not overwritten.
    route["rule_set"].as_array_mut().unwrap()[0] = json!({
        "type": "local",
        "tag": "geosite-private",
        "format": "source",
        "path": "/tmp/user.json"
    });
    ensure_cn_rule_sets(&mut route);
    let arr = route["rule_set"].as_array().unwrap();
    assert_eq!(arr.len(), 5, "same-tag entry must not be duplicated");
    assert_eq!(arr[0]["type"], "local");
    assert_eq!(arr[0]["path"], "/tmp/user.json");
}

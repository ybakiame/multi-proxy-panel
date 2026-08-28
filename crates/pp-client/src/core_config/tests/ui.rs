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

fn features_with_ui(ui: &str) -> PanelFeatures {
    PanelFeatures {
        clash_api_ui: ui.to_string(),
        ..singbox_features()
    }
}

/// Three UI choices sing-box injection assertion.
#[test]
fn apply_singbox_panel_features_injects_external_ui_download_url() {
    let cases = [
        (
            "yacd",
            "https://github.com/haishanh/yacd/archive/gh-pages.zip",
        ),
        (
            "zashboard",
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip",
        ),
        (
            "metacubexd",
            "https://github.com/MetaCubeX/metacubexd/archive/gh-pages.zip",
        ),
    ];
    for (ui, url) in cases {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut cfg, CoreType::SingBox, &features_with_ui(ui));

        assert_eq!(
            cfg["experimental"]["clash_api"]["external_ui"],
            format!("ui-{}", normalized_clash_api_ui(ui)),
            "UI choice {ui}"
        );
        assert_eq!(
            cfg["experimental"]["clash_api"]["external_ui_download_url"], url,
            "UI choice {ui}"
        );
    }
}

/// Three UI choices mihomo injection assertion.
#[test]
fn apply_mihomo_panel_features_injects_external_ui_download_url() {
    let cases = [
        (
            "yacd",
            "https://github.com/haishanh/yacd/archive/gh-pages.zip",
        ),
        (
            "zashboard",
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip",
        ),
        (
            "metacubexd",
            "https://github.com/MetaCubeX/metacubexd/archive/gh-pages.zip",
        ),
    ];
    for (ui, url) in cases {
        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        apply_panel_features(&mut cfg, CoreType::Mihomo, &features_with_ui(ui));

        assert_eq!(
            cfg["external-ui"],
            format!("ui-{}", normalized_clash_api_ui(ui)),
            "UI choice {ui}"
        );
        assert_eq!(cfg["external-ui-url"], url, "UI choice {ui}");
    }
}

/// Android (`is_android=true`): mihomo synchronously downloads external-ui panel zip in ApplyConfig path,
/// blocking setup and slowing startup; this app's built-in UI panel has no value.
/// Android branch only writes external-controller + secret, not external-ui /
/// external-ui-url, and removes template/override's three panel UI keys (including
/// external-ui-name). Desktop behavior covered by existing tests, not affected.
#[test]
fn apply_mihomo_panel_features_android_omits_external_ui_and_keeps_controller() {
    let yaml = r#"
mixed-port: 17890
external-controller: 0.0.0.0:60000
external-ui: ui
external-ui-url: https://github.com/haishanh/yacd/archive/gh-pages.zip
external-ui-name: yacd
proxies:
  - name: n1
    type: direct
rules:
  - MATCH,DIRECT
"#;
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    let features = PanelFeatures {
        clash_api_secret: "sekret".to_string(),
        ..singbox_features()
    };
    apply_mihomo_panel_features_impl(&mut cfg, &features, true);

    // external-controller / secret retained (Clash API rule mode hot switch dependency).
    assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
    assert_eq!(cfg["secret"], "sekret");
    // Do not write external-ui / external-ui-url; template/override's own panel UI keys removed.
    assert!(
        cfg.get("external-ui").is_none(),
        "Android should not write external-ui: {cfg}"
    );
    assert!(
        cfg.get("external-ui-url").is_none(),
        "Android should not write external-ui-url: {cfg}"
    );
    assert!(
        cfg.get("external-ui-name").is_none(),
        "template's own external-ui-name should be removed: {cfg}"
    );
}

/// Android (`is_android=true`) + empty secret: external-controller retained, secret
/// omitted, template's own panel UI keys removed.
#[test]
fn apply_mihomo_panel_features_android_omits_empty_secret_and_template_ui() {
    let yaml = "mixed-port: 17890\nexternal-ui: ui\nexternal-ui-url: https://old.example/panel.zip\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    let features = PanelFeatures {
        clash_api_secret: String::new(), // empty secret -> omit this key
        ..singbox_features()
    };
    apply_mihomo_panel_features_impl(&mut cfg, &features, true);

    assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
    assert!(cfg.get("secret").is_none());
    assert!(cfg.get("external-ui").is_none());
    assert!(cfg.get("external-ui-url").is_none());
}

/// Unknown value / empty string falls back to zashboard (mapping function + both core injection paths).
#[test]
fn clash_api_ui_unknown_falls_back_to_zashboard() {
    assert_eq!(
        clash_api_ui_download_url("unknown-ui"),
        "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
    );
    assert_eq!(
        clash_api_ui_download_url(""),
        "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
    );
    assert_eq!(
        clash_api_ui_download_url("zashboard"),
        "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
    );

    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut sb = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut sb, CoreType::SingBox, &features_with_ui("bogus"));
    assert_eq!(
        sb["experimental"]["clash_api"]["external_ui_download_url"],
        "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
    );
    assert_eq!(
        sb["experimental"]["clash_api"]["external_ui"], "ui-zashboard",
        "when falling back to zashboard, directory name should also fall back"
    );

    let yaml =
        "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut mh = compose_mihomo_config(yaml, 17890, None).unwrap();
    apply_panel_features(&mut mh, CoreType::Mihomo, &features_with_ui("bogus"));
    assert_eq!(
        mh["external-ui-url"],
        "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
    );
    assert_eq!(
        mh["external-ui"], "ui-zashboard",
        "when falling back to zashboard, directory name should also fall back"
    );
}

/// After switching choice, external_ui directory name differs — this is the key to switch taking effect:
/// core only downloads panel zip when directory does not exist, fixed `ui` directory means switching choice
/// only changes download URL, old panel in existing directory never gets re-downloaded (still old panel after restart).
/// Directory distinguished by choice triggers re-download for new choice, old directory residue does not matter.
/// Both cores consistent.
#[test]
fn switching_ui_choice_changes_external_ui_dir_for_both_cores() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut sb_yacd = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut sb_yacd, CoreType::SingBox, &features_with_ui("yacd"));
    let mut sb_zash = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(
        &mut sb_zash,
        CoreType::SingBox,
        &features_with_ui("zashboard"),
    );

    assert_eq!(
        sb_yacd["experimental"]["clash_api"]["external_ui"], "ui-yacd",
        "sing-box yacd directory"
    );
    assert_eq!(
        sb_zash["experimental"]["clash_api"]["external_ui"], "ui-zashboard",
        "sing-box zashboard directory"
    );
    assert_ne!(
        sb_yacd["experimental"]["clash_api"]["external_ui"],
        sb_zash["experimental"]["clash_api"]["external_ui"],
        "sing-box external_ui directory must differ after switching choice"
    );

    let yaml =
        "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut mh_yacd = compose_mihomo_config(yaml, 17890, None).unwrap();
    apply_panel_features(&mut mh_yacd, CoreType::Mihomo, &features_with_ui("yacd"));
    let mut mh_zash = compose_mihomo_config(yaml, 17890, None).unwrap();
    apply_panel_features(
        &mut mh_zash,
        CoreType::Mihomo,
        &features_with_ui("zashboard"),
    );

    assert_eq!(mh_yacd["external-ui"], "ui-yacd", "mihomo yacd directory");
    assert_eq!(
        mh_zash["external-ui"], "ui-zashboard",
        "mihomo zashboard directory"
    );
    assert_ne!(
        mh_yacd["external-ui"], mh_zash["external-ui"],
        "mihomo external-ui directory must differ after switching choice"
    );
}

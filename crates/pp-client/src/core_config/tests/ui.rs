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
        apply_panel_features(&mut cfg, &features_with_ui(ui));

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

/// Unknown value / empty string falls back to zashboard (mapping function + injection path).
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
    apply_panel_features(&mut sb, &features_with_ui("bogus"));
    assert_eq!(
        sb["experimental"]["clash_api"]["external_ui_download_url"],
        "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
    );
    assert_eq!(
        sb["experimental"]["clash_api"]["external_ui"], "ui-zashboard",
        "when falling back to zashboard, directory name should also fall back"
    );
}

/// After switching choice, external_ui directory name differs — this is the key to switch taking effect:
/// core only downloads panel zip when directory does not exist, fixed `ui` directory means switching choice
/// only changes download URL, old panel in existing directory never gets re-downloaded (still old panel after restart).
/// Directory distinguished by choice triggers re-download for new choice, old directory residue does not matter.
#[test]
fn switching_ui_choice_changes_external_ui_dir() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut sb_yacd = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut sb_yacd, &features_with_ui("yacd"));
    let mut sb_zash = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut sb_zash, &features_with_ui("zashboard"));

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
}

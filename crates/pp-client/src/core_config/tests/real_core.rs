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

#[test]
fn singbox_tun_clash_api_passes_real_singbox_check() {
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
    // Use non-default UI (metacubexd) to verify external_ui / external_ui_download_url injection
    // still passes real sing-box check.
    apply_panel_features(&mut cfg, CoreType::SingBox, &features_with_ui("metacubexd"));

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
        "sing-box check failed (tun + clash_api): {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn mihomo_tun_clash_api_passes_real_mihomo_check() {
    let Some(bin) = mihomo_binary() else {
        return;
    };
    let yaml =
        "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
    // Use non-default UI (metacubexd) to verify external-ui / external-ui-url injection still passes real
    // mihomo check.
    apply_panel_features(&mut cfg, CoreType::Mihomo, &features_with_ui("metacubexd"));

    let dir = tempfile::tempdir().unwrap();
    // Pre-place geoip.metadb (if exists) to avoid `mihomo -t` downloading geo data.
    if let Some(mmdb) = geoip_metadb() {
        std::fs::copy(mmdb, dir.path().join("geoip.metadb")).unwrap();
    }
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, serde_yaml::to_string(&cfg).unwrap()).unwrap();
    let out = std::process::Command::new(&bin)
        .args(["-t", "-f"])
        .arg(&path)
        .arg("-d")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "mihomo check failed (tun + clash_api): {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

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
        dns_mode: crate::config_slices::DnsMode::FollowSystem,
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
    apply_panel_features(&mut cfg, &features_with_ui("metacubexd"));

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

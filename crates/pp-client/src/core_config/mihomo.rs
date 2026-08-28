//! mihomo panel feature injection.

use serde_json::{Value, json};

use super::PanelFeatures;

/// mihomo panel injection:
///
/// - `mode` → written to top-level `mode: <rule_mode>` (invalid values fall back to `rule`;
///   when template/override already has `mode`, replaced by settings). sing-box has no composition-level
///   mode field, rule mode switched by [`push_clash_mode`] at runtime;
/// - `tun_enabled` → `tun = {enable: true, stack, auto-route, auto-detect-interface: true,
///   dns-hijack: ["any:53"]}` (when template already has `tun`, replaced wholesale);
/// - `clash_api_enabled` → `external-controller: 127.0.0.1:port`,
///   `external-ui: ui-<choice>` + `external-ui-url: <by choice>`, write `secret` key when
///   non-empty (empty string omitted; when template already has `external-controller`, replaced
///   by settings).
///
///   `external-ui` directory name distinguished by choice (`ui-yacd` / `ui-zashboard` /
///   `ui-metacubexd`), same reason as sing-box: mihomo only downloads panel zip when
///   `external-ui` directory does not exist (`external-ui-url`), fixed `ui` directory means
///   switching choice won't trigger re-download, restart still shows old panel; after directory
///   distinction, switching choice goes to new directory and re-downloads, old directory残留
///   does not affect. URL path remains `/ui`.
///
/// **Platform difference (Android)**: `external-ui` / `external-ui-url` makes mihomo
/// synchronously download panel zip in ApplyConfig path (several MB, first time needs proxy/GitHub),
/// blocking setup → startTun not executing → Android startup very slow. This app has its own
/// frontend UI, panel has no value, so Android branch only writes `external-controller`
/// (Clash API, rule mode hot switch [`push_clash_mode`] depends on it) and `secret` (when non-empty),
/// does not write `external-ui` / `external-ui-url`, and removes template/override's own
/// `external-ui` / `external-ui-url` / `external-ui-name` keys (to prevent subscription template's
/// own panel URL from also triggering download). Desktop behavior unchanged.
pub fn apply_mihomo_panel_features(composed: &mut Value, features: &PanelFeatures) {
    apply_mihomo_panel_features_impl(composed, features, cfg!(target_os = "android"));
}

/// [`apply_mihomo_panel_features`] private implementation, `is_android` is platform judgment parameter
/// (production path passes `cfg!(target_os = "android")` from [`apply_mihomo_panel_features`];
/// tests can directly pass `true` / `false` to override both branches, see file-level `#[cfg(test)]`).
pub(crate) fn apply_mihomo_panel_features_impl(
    composed: &mut Value,
    features: &PanelFeatures,
    is_android: bool,
) {
    let Some(obj) = composed.as_object_mut() else {
        return;
    };

    // Rule mode persistence: write to mihomo top-level `mode` (when template already has, replaced by settings).
    obj.insert(
        "mode".to_string(),
        Value::String(super::normalized_rule_mode(&features.rule_mode).to_string()),
    );

    if features.tun_enabled {
        obj.insert(
            "tun".to_string(),
            json!({
                "enable": true,
                "stack": features.tun_stack,
                "auto-route": features.tun_auto_route,
                "auto-detect-interface": true,
                "dns-hijack": ["any:53"],
            }),
        );
    }

    if features.clash_api_enabled {
        obj.insert(
            "external-controller".to_string(),
            Value::String(format!("127.0.0.1:{}", features.clash_api_port)),
        );
        if is_android {
            // Android: don't write external-ui / external-ui-url, and remove template/override's own
            // panel UI keys (external-ui / external-ui-url / external-ui-name) —
            // external-ui synchronously downloads panel zip in ApplyConfig path, blocking setup and
            // slowing startup; this app has its own UI, panel has no value.
            // external-controller retained for Clash API use (rule mode hot switch push_clash_mode depends on it).
            for key in ["external-ui", "external-ui-url", "external-ui-name"] {
                obj.remove(key);
            }
        } else {
            // Desktop: external-ui directory name distinguished by choice (`ui-<choice>`, unknown falls back
            // to zashboard). mihomo only downloads panel zip when external-ui directory does not exist
            // (external-ui-url), fixed `ui` directory means switching choice won't trigger re-download,
            // restart still shows old panel; after directory distinction, switching choice goes to new
            // directory and re-downloads, old directory残留 does not affect. URL path remains `/ui`.
            obj.insert(
                "external-ui".to_string(),
                Value::String(format!(
                    "ui-{}",
                    super::normalized_clash_api_ui(&features.clash_api_ui)
                )),
            );
            obj.insert(
                "external-ui-url".to_string(),
                Value::String(super::clash_api_ui_download_url(&features.clash_api_ui).to_string()),
            );
        }
        if features.clash_api_secret.is_empty() {
            obj.remove("secret");
        } else {
            obj.insert(
                "secret".to_string(),
                Value::String(features.clash_api_secret.clone()),
            );
        }
    }
}

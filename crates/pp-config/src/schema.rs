//! sing-box JSON Schema validation for generated configs.

use pp_common::{PanelError, PanelResult};
use serde_json::Value;
use std::sync::OnceLock;

const SCHEMA: &str = include_str!("../schema/singbox.schema.json");

fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value = serde_json::from_str(SCHEMA).expect("invalid sing-box schema JSON");
        jsonschema::validator_for(&schema).expect("failed to compile sing-box JSON Schema")
    })
}

/// Validate a generated sing-box config against the official JSON Schema.
/// Returns a `Config` error listing the first few violations on failure.
pub fn validate_singbox_config(config: &Value) -> PanelResult<()> {
    let errors: Vec<String> = validator()
        .iter_errors(config)
        .take(5)
        .map(|e| format!("{} at {}", e, e.instance_path))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(PanelError::Config(format!(
            "sing-box config failed schema validation: {}",
            errors.join("; ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn minimal_valid_config_passes() {
        let config = json!({
            "inbounds": [],
            "outbounds": [{"type": "direct", "tag": "direct"}]
        });
        assert!(validate_singbox_config(&config).is_ok());
    }

    #[test]
    fn obviously_invalid_config_fails() {
        // outbounds must be an array of objects, not nested arrays
        let config = json!({
            "inbounds": [],
            "outbounds": [[]]
        });
        let err = validate_singbox_config(&config).unwrap_err();
        assert!(
            err.to_string()
                .contains("sing-box config failed schema validation")
        );
    }

    /// Validate the output of the sing-box config builder (at least one
    /// vless_reality + hysteria2 node) passes the official schema.
    #[test]
    fn builder_full_config_passes_schema() {
        use crate::builder::ConfigBuilder;
        let config = crate::singbox::SingBoxConfigBuilder
            .build_full_config(&[
                crate::InboundConfig {
                    tag: "vless-reality-in".into(),
                    protocol: pp_common::ProtocolType::VlessReality,
                    listen: "::".into(),
                    port: 443,
                    settings: json!({
                        "tag": "vless-reality-in",
                        "listen": "::",
                        "port": 443,
                        "clients": [{"id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4", "email": "alice"}],
                        "flow": "xtls-rprx-vision",
                        "reality_dest": "example.com:443",
                        "reality_server_names": "example.com",
                        "reality_private_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                        "reality_short_id": "0123456789abcdef",
                    }),
                    tls: None,
                    sniffing: None,
                    core_version: None,
                },
                crate::InboundConfig {
                    tag: "hy2-in".into(),
                    protocol: pp_common::ProtocolType::Hysteria2,
                    listen: "::".into(),
                    port: 8444,
                    settings: json!({
                        "tag": "hy2-in",
                        "listen": "::",
                        "port": 8444,
                        "clients": [{"name": "alice", "password": "secret"}],
                    }),
                    tls: Some(json!({ "certFile": "/tmp/cert.pem", "keyFile": "/tmp/key.pem" })),
                    sniffing: None,
                    core_version: None,
                },
            ])
            .unwrap();
        match validate_singbox_config(&config) {
            Ok(()) => {} // expected
            Err(e) => {
                // Report precisely which fields fail
                panic!(
                    "builder full config FAILED official schema:\n{}\n\nconfig:\n{}",
                    e,
                    serde_json::to_string_pretty(&config).unwrap()
                );
            }
        }
    }

    /// Validate the builtin subscription template rendered with 2 sample
    /// nodes passes the official schema.
    #[test]
    fn builtin_template_render_passes_schema() {
        let template = r#"{
            "outbounds": [
                "<OUTBOUND_REPLACE>",
                { "type": "direct", "tag": "direct" }
            ]
        }"#;
        let outbounds = vec![
            json!({"type": "vless", "tag": "node1", "server": "1.2.3.4", "server_port": 443}),
            json!({"type": "hysteria2", "tag": "node2", "server": "5.6.7.8", "server_port": 8443}),
        ];
        let names: Vec<String> = vec!["node1".into(), "node2".into()];
        // Inline the render_template logic (it's in pp-subscription, not here)
        let outbounds_json = serde_json::to_string(&outbounds).unwrap();
        let outbounds_inner = outbounds_json
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(&outbounds_json);
        let names_json = serde_json::to_string(&names).unwrap();
        let names_inner = names_json
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(&names_json);

        let rendered_str = template
            .replace("\"<OUTBOUND_REPLACE>\"", outbounds_inner)
            .replace("\"<NODE_REPLACE>\"", names_inner);
        let config: Value = serde_json::from_str(&rendered_str).unwrap();

        match validate_singbox_config(&config) {
            Ok(()) => {}
            Err(e) => {
                panic!(
                    "builtin template render FAILED official schema:\n{}\n\nconfig:\n{}",
                    e,
                    serde_json::to_string_pretty(&config).unwrap()
                );
            }
        }
    }
}

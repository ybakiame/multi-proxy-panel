//! Relay (chain) outbound builders for server-side domain routing.

use pp_common::{PanelError, PanelResult, ProtocolType};
use serde_json::{Value, json};

/// Inputs describing one relay hop.
pub struct RelayHop<'a> {
    pub tag: &'a str, // outbound tag, e.g. "relay-a1b2c3d4"
    pub protocol: ProtocolType,
    pub settings: &'a Value, // protocol_config.settings of the EXIT binding
    pub server: &'a str,     // exit node domain or address
    pub port: u16,           // exit protocol config listen_port
    pub credential: &'a str, // relay system client id (uuid) — uuid for vless, password for hysteria2/anytls
    pub tls_sni: Option<&'a str>, // resolved TLS SNI for hysteria2/anytls (cert domain); ignored for vless_reality
}

/// Build a sing-box outbound for the relay hop.
pub fn build_singbox_outbound(hop: &RelayHop) -> PanelResult<Value> {
    match hop.protocol {
        ProtocolType::VlessReality => {
            let public_key = str_setting(hop.settings, &["public_key"]).ok_or_else(|| {
                PanelError::Config("vless_reality relay requires settings.public_key".into())
            })?;
            let server_name =
                first_string_setting(hop.settings, &["reality_server_names", "server_names"])
                    .ok_or_else(|| {
                        PanelError::Config("vless_reality relay requires server_names".into())
                    })?;
            let short_id = first_string_setting(hop.settings, &["reality_short_id", "short_id"])
                .unwrap_or_default();
            let flow = str_setting(hop.settings, &["flow"]).unwrap_or_default();
            let mut tls = json!({
                "enabled": true,
                "server_name": server_name,
                "reality": { "enabled": true, "public_key": public_key },
            });
            if !short_id.is_empty() {
                tls["reality"]["short_id"] = json!(short_id);
            }
            let mut out = json!({
                "type": "vless",
                "tag": hop.tag,
                "server": hop.server,
                "server_port": hop.port,
                "uuid": hop.credential,
                "tls": tls,
            });
            if !flow.is_empty() {
                out["flow"] = json!(flow);
            }
            Ok(out)
        }
        ProtocolType::Hysteria2 => {
            let sni = hop
                .tls_sni
                .ok_or_else(|| PanelError::Config("hysteria2 relay requires TLS SNI".into()))?;
            let mut out = json!({
                "type": "hysteria2",
                "tag": hop.tag,
                "server": hop.server,
                "server_port": hop.port,
                "password": hop.credential,
                "tls": { "enabled": true, "server_name": sni },
                "up_mbps": hop.settings.get("up_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
                "down_mbps": hop.settings.get("down_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
            });
            let obfs_type =
                str_setting(hop.settings, &["obfs_type"]).unwrap_or_else(|| "none".into());
            let obfs_password = str_setting(hop.settings, &["obfs_password"]).unwrap_or_default();
            if obfs_type != "none" && !obfs_password.is_empty() {
                out["obfs"] = json!({ "type": obfs_type, "password": obfs_password });
            }
            Ok(out)
        }
        ProtocolType::Anytls => {
            let sni = hop
                .tls_sni
                .ok_or_else(|| PanelError::Config("anytls relay requires TLS SNI".into()))?;
            Ok(json!({
                "type": "anytls",
                "tag": hop.tag,
                "server": hop.server,
                "server_port": hop.port,
                "password": hop.credential,
                "tls": { "enabled": true, "server_name": sni },
            }))
        }
        other => Err(PanelError::Config(format!(
            "protocol {:?} cannot be a relay exit",
            other
        ))),
    }
}

/// Build a mihomo proxy entry for the relay hop.
pub fn build_mihomo_proxy(hop: &RelayHop) -> PanelResult<Value> {
    match hop.protocol {
        ProtocolType::VlessReality => {
            let public_key = str_setting(hop.settings, &["public_key"]).ok_or_else(|| {
                PanelError::Config("vless_reality relay requires settings.public_key".into())
            })?;
            let server_name =
                first_string_setting(hop.settings, &["reality_server_names", "server_names"])
                    .ok_or_else(|| {
                        PanelError::Config("vless_reality relay requires server_names".into())
                    })?;
            let short_id = first_string_setting(hop.settings, &["reality_short_id", "short_id"])
                .unwrap_or_default();
            let flow = str_setting(hop.settings, &["flow"]).unwrap_or_default();
            let mut reality_opts = json!({ "public-key": public_key });
            if !short_id.is_empty() {
                reality_opts["short-id"] = json!(short_id);
            }
            let mut out = json!({
                "name": hop.tag,
                "type": "vless",
                "server": hop.server,
                "port": hop.port,
                "uuid": hop.credential,
                "network": "tcp",
                "tls": true,
                "servername": server_name,
                "reality-opts": reality_opts,
            });
            if !flow.is_empty() {
                out["flow"] = json!(flow);
            }
            Ok(out)
        }
        ProtocolType::Hysteria2 => {
            let sni = hop
                .tls_sni
                .ok_or_else(|| PanelError::Config("hysteria2 relay requires TLS SNI".into()))?;
            let mut out = json!({
                "name": hop.tag,
                "type": "hysteria2",
                "server": hop.server,
                "port": hop.port,
                "password": hop.credential,
                "sni": sni,
                "up": hop.settings.get("up_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
                "down": hop.settings.get("down_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
            });
            let obfs_type =
                str_setting(hop.settings, &["obfs_type"]).unwrap_or_else(|| "none".into());
            let obfs_password = str_setting(hop.settings, &["obfs_password"]).unwrap_or_default();
            if obfs_type != "none" && !obfs_password.is_empty() {
                out["obfs"] = json!(obfs_type);
                out["obfs-password"] = json!(obfs_password);
            }
            Ok(out)
        }
        ProtocolType::Anytls => {
            let sni = hop
                .tls_sni
                .ok_or_else(|| PanelError::Config("anytls relay requires TLS SNI".into()))?;
            Ok(json!({
                "name": hop.tag,
                "type": "anytls",
                "server": hop.server,
                "port": hop.port,
                "password": hop.credential,
                "sni": sni,
            }))
        }
        other => Err(PanelError::Config(format!(
            "protocol {:?} cannot be a relay exit",
            other
        ))),
    }
}

/// First non-empty string among several keys (string values only).
fn str_setting(settings: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|k| settings.get(*k).and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .find(|s| !s.is_empty())
}

/// First non-empty string among keys that may hold a string, a comma-separated
/// string, or an array of strings.
fn first_string_setting(settings: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        match settings.get(*key) {
            Some(Value::String(s)) if !s.is_empty() => {
                return Some(s.split(',').next().unwrap_or(s).trim().to_string());
            }
            Some(Value::Array(arr)) => {
                if let Some(s) = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .find(|s| !s.is_empty())
                {
                    return Some(s.to_string());
                }
            }
            _ => continue,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_common::ProtocolType;
    use serde_json::json;

    fn vless_settings() -> Value {
        json!({
            "public_key": "QAg3gSuBz2bmyFszUAsL6dalimBzfSbvYkf1LPP44Rs",
            "server_names": ["www.samsung.com"],
            "short_id": "915d3f92",
            "flow": "xtls-rprx-vision"
        })
    }

    fn hysteria2_settings() -> Value {
        json!({
            "up_mbps": 100,
            "down_mbps": 100,
            "obfs_type": "salamander",
            "obfs_password": "pw"
        })
    }

    #[test]
    fn test_singbox_vless_reality_outbound() {
        let hop = RelayHop {
            tag: "relay-test",
            protocol: ProtocolType::VlessReality,
            settings: &vless_settings(),
            server: "exit.example.com",
            port: 443,
            credential: "some-uuid",
            tls_sni: None,
        };
        let result = build_singbox_outbound(&hop).unwrap();
        assert_eq!(result["type"], "vless");
        assert_eq!(result["tag"], "relay-test");
        assert_eq!(result["server"], "exit.example.com");
        assert_eq!(result["server_port"], 443);
        assert_eq!(result["uuid"], "some-uuid");
        assert_eq!(result["tls"]["enabled"], true);
        assert_eq!(result["tls"]["server_name"], "www.samsung.com");
        assert_eq!(result["tls"]["reality"]["enabled"], true);
        assert_eq!(
            result["tls"]["reality"]["public_key"],
            "QAg3gSuBz2bmyFszUAsL6dalimBzfSbvYkf1LPP44Rs"
        );
        assert_eq!(result["tls"]["reality"]["short_id"], "915d3f92");
        assert_eq!(result["flow"], "xtls-rprx-vision");
    }

    #[test]
    fn test_singbox_hysteria2_outbound() {
        let hop = RelayHop {
            tag: "relay-hy2",
            protocol: ProtocolType::Hysteria2,
            settings: &hysteria2_settings(),
            server: "exit.example.com",
            port: 8443,
            credential: "pass123",
            tls_sni: Some("sni.example.com"),
        };
        let result = build_singbox_outbound(&hop).unwrap();
        assert_eq!(result["type"], "hysteria2");
        assert_eq!(result["tag"], "relay-hy2");
        assert_eq!(result["server"], "exit.example.com");
        assert_eq!(result["server_port"], 8443);
        assert_eq!(result["password"], "pass123");
        assert_eq!(result["tls"]["server_name"], "sni.example.com");
        assert_eq!(result["up_mbps"], 100);
        assert_eq!(result["down_mbps"], 100);
        assert_eq!(result["obfs"]["type"], "salamander");
        assert_eq!(result["obfs"]["password"], "pw");
    }

    #[test]
    fn test_mihomo_vless_reality_proxy() {
        let hop = RelayHop {
            tag: "relay-mihomo-vl",
            protocol: ProtocolType::VlessReality,
            settings: &vless_settings(),
            server: "relay.example.com",
            port: 443,
            credential: "mihomo-uuid",
            tls_sni: None,
        };
        let result = build_mihomo_proxy(&hop).unwrap();
        assert_eq!(result["name"], "relay-mihomo-vl");
        assert_eq!(result["type"], "vless");
        assert_eq!(result["server"], "relay.example.com");
        assert_eq!(result["port"], 443);
        assert_eq!(result["uuid"], "mihomo-uuid");
        assert_eq!(result["tls"], true);
        assert_eq!(result["servername"], "www.samsung.com");
        assert_eq!(
            result["reality-opts"]["public-key"],
            "QAg3gSuBz2bmyFszUAsL6dalimBzfSbvYkf1LPP44Rs"
        );
        assert_eq!(result["reality-opts"]["short-id"], "915d3f92");
        assert_eq!(result["flow"], "xtls-rprx-vision");
    }

    #[test]
    fn test_mihomo_hysteria2_proxy() {
        let hop = RelayHop {
            tag: "relay-mihomo-hy2",
            protocol: ProtocolType::Hysteria2,
            settings: &hysteria2_settings(),
            server: "hy2.example.com",
            port: 8443,
            credential: "hy2-pass",
            tls_sni: Some("hy2-sni.example.com"),
        };
        let result = build_mihomo_proxy(&hop).unwrap();
        assert_eq!(result["name"], "relay-mihomo-hy2");
        assert_eq!(result["type"], "hysteria2");
        assert_eq!(result["server"], "hy2.example.com");
        assert_eq!(result["port"], 8443);
        assert_eq!(result["password"], "hy2-pass");
        assert_eq!(result["sni"], "hy2-sni.example.com");
        assert_eq!(result["up"], 100);
        assert_eq!(result["down"], 100);
        assert_eq!(result["obfs"], "salamander");
        assert_eq!(result["obfs-password"], "pw");
    }

    #[test]
    fn test_missing_public_key_error() {
        let settings = json!({
            "server_names": ["www.example.com"],
        });
        let hop = RelayHop {
            tag: "err",
            protocol: ProtocolType::VlessReality,
            settings: &settings,
            server: "server.com",
            port: 443,
            credential: "uuid",
            tls_sni: None,
        };
        let err = build_singbox_outbound(&hop).unwrap_err();
        assert!(
            err.to_string().contains("public_key"),
            "Expected error about missing public_key, got: {err}"
        );
        let err = build_mihomo_proxy(&hop).unwrap_err();
        assert!(
            err.to_string().contains("public_key"),
            "Expected error about missing public_key, got: {err}"
        );
    }

    #[test]
    fn test_unsupported_protocol_error() {
        let settings = json!({});
        let hop = RelayHop {
            tag: "err",
            protocol: ProtocolType::VlessXhttp,
            settings: &settings,
            server: "server.com",
            port: 443,
            credential: "x",
            tls_sni: None,
        };
        let err = build_singbox_outbound(&hop).unwrap_err();
        assert!(
            err.to_string().contains("cannot be a relay exit"),
            "Expected error about unsupported protocol, got: {err}"
        );
        let err = build_mihomo_proxy(&hop).unwrap_err();
        assert!(
            err.to_string().contains("cannot be a relay exit"),
            "Expected error about unsupported protocol, got: {err}"
        );
    }
}

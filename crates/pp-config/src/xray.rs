use pp_common::{CoreType, PanelError, PanelResult, ProtocolType};
use serde_json::{json, Value};

use super::builder::{ConfigBuilder, InboundConfig};

/// Xray-core configuration builder.
pub struct XrayConfigBuilder;

impl ConfigBuilder for XrayConfigBuilder {
    fn core_type(&self) -> CoreType {
        CoreType::Xray
    }

    fn build_inbound(
        &self,
        protocol: ProtocolType,
        settings: &Value,
        tls: Option<&Value>,
    ) -> PanelResult<Value> {
        match protocol {
            ProtocolType::VlessReality | ProtocolType::VlessVision | ProtocolType::VlessXhttp => {
                build_vless_inbound(protocol, settings, tls)
            }
            ProtocolType::Vmess => build_vmess_inbound(settings, tls),
            ProtocolType::Trojan => build_trojan_inbound(settings, tls),
            ProtocolType::Shadowsocks2022 => build_shadowsocks_inbound(settings, tls),
            _ => Err(PanelError::Config(format!(
                "protocol {:?} not supported by xray",
                protocol
            ))),
        }
    }

    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value> {
        let mut xray_inbounds = Vec::with_capacity(inbounds.len());
        for inbound in inbounds {
            xray_inbounds.push(self.build_inbound(
                inbound.protocol,
                &inbound.settings,
                inbound.tls.as_ref(),
            )?);
        }

        Ok(json!({
            "log": {
                "loglevel": "warning"
            },
            "inbounds": xray_inbounds,
            "outbounds": [
                {
                    "protocol": "freedom",
                    "tag": "direct"
                }
            ]
        }))
    }
}

fn build_vless_inbound(
    protocol: ProtocolType,
    settings: &Value,
    tls: Option<&Value>,
) -> PanelResult<Value> {
    let mut inbound = json!({
        "protocol": "vless",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": settings.get("clients").cloned().unwrap_or(json!([])),
            "decryption": "none"
        },
        "streamSettings": {
            "network": stream_settings_network(&protocol),
            "security": if tls.is_some() { "tls" } else { "none" },
        },
        "sniffing": {
            "enabled": true,
            "destOverride": ["http", "tls", "quic"]
        }
    });

    if let Some(tls_cfg) = tls {
        inbound["streamSettings"]["tlsSettings"] = tls_cfg.clone();
        // REALITY specific
        if protocol == ProtocolType::VlessReality {
            inbound["streamSettings"]["security"] = "reality".into();
            if let Some(reality) = tls_cfg.get("reality") {
                inbound["streamSettings"]["realitySettings"] = reality.clone();
            }
        }
    }

    Ok(inbound)
}

fn build_vmess_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let mut inbound = json!({
        "protocol": "vmess",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": settings.get("clients").cloned().unwrap_or(json!([]))
        },
        "streamSettings": {
            "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp"),
            "security": if tls.is_some() { "tls" } else { "none" },
        },
        "sniffing": {
            "enabled": true,
            "destOverride": ["http", "tls", "quic"]
        }
    });

    if let Some(tls_cfg) = tls {
        inbound["streamSettings"]["tlsSettings"] = tls_cfg.clone();
    }

    Ok(inbound)
}

fn build_trojan_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    if tls.is_none() {
        return Err(PanelError::Validation(
            "Trojan requires TLS configuration".into(),
        ));
    }

    Ok(json!({
        "protocol": "trojan",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": settings.get("clients").cloned().unwrap_or(json!([]))
        },
        "streamSettings": {
            "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp"),
            "security": "tls",
            "tlsSettings": tls.unwrap()
        }
    }))
}

fn build_shadowsocks_inbound(settings: &Value, _tls: Option<&Value>) -> PanelResult<Value> {
    Ok(json!({
        "protocol": "shadowsocks",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(8388),
        "settings": {
            "method": settings.get("method").and_then(|v| v.as_str()).unwrap_or("2022-blake3-aes-128-gcm"),
            "password": settings.get("password").and_then(|v| v.as_str()).unwrap_or(""),
            "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp,udp")
        }
    }))
}

fn stream_settings_network(protocol: &ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::VlessReality | ProtocolType::VlessVision => "tcp",
        ProtocolType::VlessXhttp => "xhttp",
        _ => "tcp",
    }
}

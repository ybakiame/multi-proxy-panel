use pp_common::{CoreType, PanelError, PanelResult, ProtocolType};
use serde_json::{json, Value};

use super::builder::{ConfigBuilder, InboundConfig};

/// sing-box configuration builder.
pub struct SingBoxConfigBuilder;

impl ConfigBuilder for SingBoxConfigBuilder {
    fn core_type(&self) -> CoreType {
        CoreType::SingBox
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
            ProtocolType::Hysteria2 => build_hysteria2_inbound(settings, tls),
            ProtocolType::TuicV5 => build_tuic_inbound(settings, tls),
        }
    }

    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value> {
        let mut sb_inbounds = Vec::with_capacity(inbounds.len());
        for inbound in inbounds {
            sb_inbounds.push(self.build_inbound(
                inbound.protocol,
                &inbound.settings,
                inbound.tls.as_ref(),
            )?);
        }

        Ok(json!({
            "log": {
                "level": "warning",
                "timestamp": true
            },
            "inbounds": sb_inbounds,
            "outbounds": [
                {
                    "type": "direct",
                    "tag": "direct"
                }
            ],
            "route": {
                "auto_detect_interface": true
            }
        }))
    }
}

fn build_vless_inbound(
    protocol: ProtocolType,
    settings: &Value,
    tls: Option<&Value>,
) -> PanelResult<Value> {
    let mut inbound = json!({
        "type": "vless",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("vless-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": settings.get("clients").cloned().unwrap_or(json!([])),
        "multiplex": settings.get("multiplex").cloned(),
    });

    // Remove null multiplex
    if inbound["multiplex"].is_null() {
        inbound.as_object_mut().unwrap().remove("multiplex");
    }

    if protocol == ProtocolType::VlessReality {
        if let Some(tls_cfg) = tls {
            inbound["tls"] = json!({
                "enabled": true,
                "server_name": tls_cfg.get("serverName").and_then(|v| v.as_str()).unwrap_or(""),
                "reality": {
                    "enabled": true,
                    "handshake": {
                        "server": tls_cfg.get("reality").and_then(|r| r.get("dest")).and_then(|v| v.as_str()).unwrap_or(""),
                        "server_port": 443
                    },
                    "private_key": tls_cfg.get("reality").and_then(|r| r.get("privateKey")).and_then(|v| v.as_str()).unwrap_or(""),
                    "short_id": tls_cfg.get("reality").and_then(|r| r.get("shortIds")).and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|v| v.as_str()).unwrap_or("")
                }
            });
        }
    } else if let Some(tls_cfg) = tls {
        inbound["tls"] = json!({
            "enabled": true,
            "server_name": tls_cfg.get("serverName").and_then(|v| v.as_str()).unwrap_or(""),
            "certificate_path": tls_cfg.get("certFile").and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls_cfg.get("keyFile").and_then(|v| v.as_str()).unwrap_or(""),
        });
    }

    Ok(inbound)
}

fn build_vmess_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let mut inbound = json!({
        "type": "vmess",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("vmess-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": settings.get("clients").cloned().unwrap_or(json!([])),
    });

    if let Some(tls_cfg) = tls {
        inbound["tls"] = json!({
            "enabled": true,
            "server_name": tls_cfg.get("serverName").and_then(|v| v.as_str()).unwrap_or(""),
        });
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
        "type": "trojan",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("trojan-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": settings.get("clients").cloned().unwrap_or(json!([])),
        "tls": {
            "enabled": true,
            "certificate_path": tls.and_then(|t| t.get("certFile")).and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls.and_then(|t| t.get("keyFile")).and_then(|v| v.as_str()).unwrap_or(""),
        }
    }))
}

fn build_shadowsocks_inbound(settings: &Value, _tls: Option<&Value>) -> PanelResult<Value> {
    Ok(json!({
        "type": "shadowsocks",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("ss-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(8388),
        "method": settings.get("method").and_then(|v| v.as_str()).unwrap_or("2022-blake3-aes-128-gcm"),
        "password": settings.get("password").and_then(|v| v.as_str()).unwrap_or(""),
        "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp")
    }))
}

fn build_hysteria2_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    if tls.is_none() {
        return Err(PanelError::Validation(
            "Hysteria2 requires TLS configuration".into(),
        ));
    }

    Ok(json!({
        "type": "hysteria2",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("hy2-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": settings.get("clients").cloned().unwrap_or(json!([])),
        "tls": {
            "enabled": true,
            "certificate_path": tls.and_then(|t| t.get("certFile")).and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls.and_then(|t| t.get("keyFile")).and_then(|v| v.as_str()).unwrap_or(""),
        },
        "up_mbps": settings.get("up_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
        "down_mbps": settings.get("down_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
    }))
}

fn build_tuic_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    if tls.is_none() {
        return Err(PanelError::Validation(
            "TUIC requires TLS configuration".into(),
        ));
    }

    Ok(json!({
        "type": "tuic",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("tuic-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": settings.get("clients").cloned().unwrap_or(json!([])),
        "tls": {
            "enabled": true,
            "certificate_path": tls.and_then(|t| t.get("certFile")).and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls.and_then(|t| t.get("keyFile")).and_then(|v| v.as_str()).unwrap_or(""),
        },
        "congestion_control": settings.get("congestion_control").and_then(|v| v.as_str()).unwrap_or("bbr"),
    }))
}

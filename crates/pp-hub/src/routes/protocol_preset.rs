//! Protocol configuration presets for supported secure protocols.
//!
//! Presets bundle a base protocol + transport + security into ready-to-use
//! `protocol_configs` rows, reducing manual JSON editing.

use axum::{Json, extract::State};
use pp_common::{CoreType, ProtocolType};
use pp_db::entities::protocol_config;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

/// Available one-click protocol presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolPreset {
    /// VLESS + REALITY + vision flow
    VlessReality,
    /// VLESS + XHTTP + TLS (mihomo)
    VlessXhttpTls,
    /// Hysteria2 (sing-box only)
    Hysteria2,
    /// AnyTLS (sing-box only)
    Anytls,
}

/// Description of a preset for the frontend.
#[derive(Debug, serde::Serialize)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub protocol_type: String,
    pub core_type: String,
}

/// Return the list of available presets.
pub fn list_presets() -> Vec<PresetInfo> {
    vec![
        PresetInfo {
            id: "vless_reality".to_string(),
            name: "VLESS + REALITY".to_string(),
            description: "Recommended. VLESS with REALITY handshake and xtls-rprx-vision flow."
                .to_string(),
            protocol_type: ProtocolType::VlessReality.to_string(),
            core_type: CoreType::SingBox.to_string(),
        },
        PresetInfo {
            id: "vless_xhttp_tls".to_string(),
            name: "VLESS + XHTTP + TLS".to_string(),
            description: "VLESS over XHTTP (HTTPUpgrade), mihomo.".to_string(),
            protocol_type: ProtocolType::VlessXhttp.to_string(),
            core_type: CoreType::Mihomo.to_string(),
        },
        PresetInfo {
            id: "hysteria2".to_string(),
            name: "Hysteria2".to_string(),
            description: "Hysteria2 with TLS, sing-box only.".to_string(),
            protocol_type: ProtocolType::Hysteria2.to_string(),
            core_type: CoreType::SingBox.to_string(),
        },
        PresetInfo {
            id: "anytls".to_string(),
            name: "AnyTLS".to_string(),
            description: "AnyTLS with TLS, sing-box only.".to_string(),
            protocol_type: ProtocolType::Anytls.to_string(),
            core_type: CoreType::SingBox.to_string(),
        },
    ]
}

/// Expand a preset into a ready-to-insert protocol config payload.
///
/// `domain` is used for TLS/REALITY SNI. `port` defaults are preset-specific.
pub fn expand_preset(
    preset: ProtocolPreset,
    name: &str,
    domain: Option<&str>,
    port: Option<u16>,
) -> Value {
    let sni = domain.unwrap_or("example.com");
    let default_port = default_port_for_preset(preset);
    let listen_port = port.unwrap_or(default_port) as u64;

    let (protocol_type, core_type, settings, tls_settings) = match preset {
        ProtocolPreset::VlessReality => {
            let settings = json!({
                "clients": [],
                "decryption": "none",
                "flow": "xtls-rprx-vision",
                "network": "tcp",
                "reality_dest": format!("{}:443", sni),
                "reality_server_names": sni,
                "reality_private_key": "",
                "reality_short_id": pp_common::generate_short_id(),
                "fingerprint": "chrome",
            });
            (
                ProtocolType::VlessReality,
                CoreType::SingBox,
                settings,
                None,
            )
        }
        ProtocolPreset::VlessXhttpTls => {
            let settings = json!({
                "clients": [],
                "decryption": "none",
                "network": "xhttp",
                "xhttp_path": "/xhttp",
                "xhttp_host": sni,
                "xhttp_mode": "auto",
            });
            let tls = default_tls_settings(sni);
            (
                ProtocolType::VlessXhttp,
                CoreType::Mihomo,
                settings,
                Some(tls),
            )
        }
        ProtocolPreset::Hysteria2 => {
            let settings = json!({
                "clients": [],
                "up_mbps": 100,
                "down_mbps": 100,
                "obfs_type": "none",
                "obfs_password": "",
            });
            let tls = default_tls_settings(sni);
            (
                ProtocolType::Hysteria2,
                CoreType::SingBox,
                settings,
                Some(tls),
            )
        }
        ProtocolPreset::Anytls => {
            let settings = json!({
                "clients": [],
                "masquerade": "",
            });
            let tls = default_tls_settings(sni);
            (ProtocolType::Anytls, CoreType::SingBox, settings, Some(tls))
        }
    };

    json!({
        "name": name,
        "protocol_type": protocol_type.to_string(),
        "core_type": core_type.to_string(),
        "listen_port": listen_port,
        "listen_address": "0.0.0.0",
        "settings": settings,
        "tls_settings": tls_settings,
    })
}

fn default_port_for_preset(_preset: ProtocolPreset) -> u16 {
    443
}

fn default_tls_settings(sni: &str) -> Value {
    json!({
        "serverName": sni,
        "certFile": "/path/to/cert.pem",
        "keyFile": "/path/to/key.pem",
    })
}

#[derive(serde::Deserialize)]
pub struct ApplyPresetPayload {
    pub preset: String,
    pub name: String,
    pub domain: Option<String>,
    pub port: Option<u16>,
}

/// GET /api/v1/protocols/presets — list available presets.
pub async fn list_available_presets() -> ApiResult<Value> {
    let presets: Vec<Value> = list_presets()
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "description": p.description,
                "protocol_type": p.protocol_type,
                "core_type": p.core_type,
            })
        })
        .collect();
    Ok(ApiResponse::new(json!({ "presets": presets })))
}

/// POST /api/v1/protocols/presets — create a protocol config from a preset.
pub async fn apply_preset(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ApplyPresetPayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "preset name is required",
        ));
    }

    let preset = payload
        .preset
        .parse::<ProtocolPreset>()
        .map_err(|_| ApiError::bad_request("invalid_preset", "unknown preset"))?;

    let expanded = expand_preset(
        preset,
        &payload.name,
        payload.domain.as_deref(),
        payload.port,
    );

    let protocol_type = expanded["protocol_type"].as_str().unwrap_or("").to_string();
    let core_type = expanded["core_type"].as_str().unwrap_or("").to_string();
    let listen_port = expanded["listen_port"].as_u64().unwrap_or(443) as i32;
    let listen_address = expanded["listen_address"]
        .as_str()
        .unwrap_or("0.0.0.0")
        .to_string();
    let settings = expanded["settings"].clone();
    let tls_settings = expanded["tls_settings"].clone();

    let active = protocol_config::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        protocol_type: Set(protocol_type),
        core_type: Set(core_type),
        listen_port: Set(listen_port),
        listen_address: Set(listen_address),
        settings: Set(settings),
        tls_settings: Set(Some(tls_settings).filter(|v| !v.is_null())),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(json!({
        "id": inserted.id,
        "name": inserted.name,
        "protocol_type": inserted.protocol_type,
        "core_type": inserted.core_type,
        "listen_port": inserted.listen_port,
        "listen_address": inserted.listen_address,
        "settings": inserted.settings,
        "tls_settings": inserted.tls_settings,
    })))
}

impl std::str::FromStr for ProtocolPreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "vless_reality" => Ok(ProtocolPreset::VlessReality),
            "vless_xhttp_tls" => Ok(ProtocolPreset::VlessXhttpTls),
            "hysteria2" => Ok(ProtocolPreset::Hysteria2),
            "anytls" => Ok(ProtocolPreset::Anytls),
            _ => Err(format!("unknown preset: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vless_reality_preset_has_reality_fields() {
        let payload = expand_preset(
            ProtocolPreset::VlessReality,
            "test",
            Some("example.com"),
            None,
        );
        assert_eq!(payload["protocol_type"], "vless_reality");
        assert_eq!(payload["core_type"], "sing-box");
        assert_eq!(payload["listen_port"], 443);
        assert!(payload["settings"]["reality_private_key"].is_string());
    }

    #[test]
    fn hysteria2_preset_has_tls_and_users() {
        let payload = expand_preset(
            ProtocolPreset::Hysteria2,
            "test",
            Some("cdn.example.com"),
            None,
        );
        assert_eq!(payload["protocol_type"], "hysteria2");
        assert_eq!(payload["core_type"], "sing-box");
        assert_eq!(payload["settings"]["up_mbps"], 100);
        assert_eq!(payload["tls_settings"]["serverName"], "cdn.example.com");
    }
}

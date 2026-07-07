use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{
    Alert, ConfirmDialog, FormInput, FormSelect, FormTextarea, Modal, Pagination,
};

fn gen_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        "-".to_string()
    } else if s.len() <= 8 {
        "••••".to_string()
    } else {
        format!("{}••••{}", &s[..4], &s[s.len() - 4..])
    }
}

fn protocol_settings_summary(pt: &str, settings: &Value) -> String {
    match pt {
        "vless_reality" | "vless_vision" | "vless_xhttp" | "vmess" | "tuic" | "tuic_v5" => settings
            .get("uuid")
            .and_then(|v| v.as_str())
            .map(mask_secret)
            .unwrap_or_else(|| "-".to_string()),
        "trojan" | "hysteria2" | "anytls" => settings
            .get("password")
            .and_then(|v| v.as_str())
            .map(mask_secret)
            .unwrap_or_else(|| "-".to_string()),
        "shadowsocks2022" => settings
            .get("password")
            .and_then(|v| v.as_str())
            .map(mask_secret)
            .unwrap_or_else(|| "-".to_string()),
        _ => "-".to_string(),
    }
}

fn parse_tls_settings(s: &str) -> Result<Option<Value>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|e| format!("TLS settings JSON error: {}", e))
}

#[component]
pub fn Protocols() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut protocols = use_resource(move || async move {
        api::get_protocols(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });

    let mut show_modal = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut is_edit = use_signal(|| false);
    let mut edit_id = use_signal(String::new);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    // Common fields
    let mut new_name = use_signal(String::new);
    let mut new_protocol_type = use_signal(|| "vless_reality".to_string());
    let mut new_core_type = use_signal(|| "xray".to_string());
    let mut new_listen_address = use_signal(|| "0.0.0.0".to_string());
    let mut new_listen_port = use_signal(|| "443".to_string());
    let mut new_tls_settings = use_signal(String::new);

    // VLESS fields
    let mut vless_uuid = use_signal(gen_uuid);
    let mut vless_flow = use_signal(|| "xtls-rprx-vision".to_string());

    // VLESS + REALITY fields
    let mut reality_dest = use_signal(|| "www.cloudflare.com:443".to_string());
    let mut reality_server_names = use_signal(String::new);
    let mut reality_private_key = use_signal(String::new);
    let mut reality_public_key = use_signal(String::new);
    let mut reality_short_id = use_signal(String::new);

    // VLESS + XHTTP fields
    let mut xhttp_path = use_signal(|| "/xhttp".to_string());
    let mut xhttp_host = use_signal(String::new);
    let mut xhttp_mode = use_signal(|| "auto".to_string());

    // VMess fields
    let mut vmess_uuid = use_signal(gen_uuid);
    let mut vmess_alter_id = use_signal(|| "0".to_string());

    // Trojan fields
    let mut trojan_password = use_signal(String::new);

    // Shadowsocks 2022 fields
    let mut ss_method = use_signal(|| "2022-blake3-aes-128-gcm".to_string());
    let mut ss_password = use_signal(String::new);

    // Hysteria2 fields
    let mut h2_password = use_signal(String::new);
    let mut h2_obfs_type = use_signal(|| "none".to_string());
    let mut h2_obfs_password = use_signal(String::new);
    let mut h2_up_mbps = use_signal(|| "100".to_string());
    let mut h2_down_mbps = use_signal(|| "100".to_string());

    // AnyTLS fields
    let mut anytls_password = use_signal(String::new);
    let mut anytls_masquerade = use_signal(String::new);

    // TUIC fields
    let mut tuic_uuid = use_signal(gen_uuid);
    let mut tuic_password = use_signal(String::new);
    let mut tuic_cc = use_signal(|| "cubic".to_string());

    let protocols_data = protocols
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = protocols
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let current_per_page = *per_page.read();
    let current_page = *page.read();

    let pt = new_protocol_type.read().clone();

    let mut reset_form = move || {
        new_name.set(String::new());
        new_protocol_type.set("vless_reality".to_string());
        new_core_type.set("xray".to_string());
        new_listen_address.set("0.0.0.0".to_string());
        new_listen_port.set("443".to_string());
        new_tls_settings.set(String::new());
        vless_uuid.set(gen_uuid());
        vless_flow.set("xtls-rprx-vision".to_string());
        reality_dest.set("www.cloudflare.com:443".to_string());
        reality_server_names.set(String::new());
        reality_private_key.set(String::new());
        reality_public_key.set(String::new());
        reality_short_id.set(String::new());
        xhttp_path.set("/xhttp".to_string());
        xhttp_host.set(String::new());
        xhttp_mode.set("auto".to_string());
        vmess_uuid.set(gen_uuid());
        vmess_alter_id.set("0".to_string());
        trojan_password.set(String::new());
        ss_method.set("2022-blake3-aes-128-gcm".to_string());
        ss_password.set(String::new());
        h2_password.set(String::new());
        h2_obfs_type.set("none".to_string());
        h2_obfs_password.set(String::new());
        h2_up_mbps.set("100".to_string());
        h2_down_mbps.set("100".to_string());
        anytls_password.set(String::new());
        anytls_masquerade.set(String::new());
        tuic_uuid.set(gen_uuid());
        tuic_password.set(String::new());
        tuic_cc.set("cubic".to_string());
    };

    let mut load_settings = move |protocol_type: &str, settings: &Value, tls: &Value| {
        match protocol_type {
            "vless_reality" => {
                vless_uuid.set(
                    settings
                        .get("uuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&gen_uuid())
                        .to_string(),
                );
                vless_flow.set(
                    settings
                        .get("flow")
                        .and_then(|v| v.as_str())
                        .unwrap_or("xtls-rprx-vision")
                        .to_string(),
                );
                reality_dest.set(
                    settings
                        .get("reality_dest")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("dest").and_then(|v| v.as_str()))
                        .unwrap_or("www.cloudflare.com:443")
                        .to_string(),
                );
                reality_server_names.set(
                    settings
                        .get("reality_server_names")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("server_names").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string(),
                );
                reality_private_key.set(
                    settings
                        .get("reality_private_key")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("private_key").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string(),
                );
                reality_public_key.set(
                    settings
                        .get("reality_public_key")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("public_key").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string(),
                );
                reality_short_id.set(
                    settings
                        .get("reality_short_id")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("short_id").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string(),
                );
            }
            "vless_vision" => {
                vless_uuid.set(
                    settings
                        .get("uuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&gen_uuid())
                        .to_string(),
                );
                vless_flow.set(
                    settings
                        .get("flow")
                        .and_then(|v| v.as_str())
                        .unwrap_or("xtls-rprx-vision")
                        .to_string(),
                );
            }
            "vless_xhttp" => {
                vless_uuid.set(
                    settings
                        .get("uuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&gen_uuid())
                        .to_string(),
                );
                xhttp_path.set(
                    settings
                        .get("xhttp_path")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("path").and_then(|v| v.as_str()))
                        .unwrap_or("/xhttp")
                        .to_string(),
                );
                xhttp_host.set(
                    settings
                        .get("xhttp_host")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("host").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string(),
                );
                xhttp_mode.set(
                    settings
                        .get("xhttp_mode")
                        .and_then(|v| v.as_str())
                        .or_else(|| settings.get("mode").and_then(|v| v.as_str()))
                        .unwrap_or("auto")
                        .to_string(),
                );
            }
            "vmess" => {
                vmess_uuid.set(
                    settings
                        .get("uuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&gen_uuid())
                        .to_string(),
                );
                vmess_alter_id.set(
                    settings
                        .get("alterId")
                        .and_then(|v| v.as_i64())
                        .or_else(|| settings.get("alter_id").and_then(|v| v.as_i64()))
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "0".to_string()),
                );
            }
            "trojan" => {
                trojan_password.set(
                    settings
                        .get("password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
            }
            "shadowsocks2022" => {
                ss_method.set(
                    settings
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("2022-blake3-aes-128-gcm")
                        .to_string(),
                );
                ss_password.set(
                    settings
                        .get("password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
            }
            "hysteria2" => {
                h2_password.set(
                    settings
                        .get("password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
                h2_obfs_type.set(
                    settings
                        .get("obfs_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("none")
                        .to_string(),
                );
                h2_obfs_password.set(
                    settings
                        .get("obfs_password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
                h2_up_mbps.set(
                    settings
                        .get("up_mbps")
                        .and_then(|v| v.as_u64())
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "100".to_string()),
                );
                h2_down_mbps.set(
                    settings
                        .get("down_mbps")
                        .and_then(|v| v.as_u64())
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "100".to_string()),
                );
            }
            "anytls" => {
                anytls_password.set(
                    settings
                        .get("password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
                anytls_masquerade.set(
                    settings
                        .get("masquerade")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
            }
            "tuic" | "tuic_v5" => {
                tuic_uuid.set(
                    settings
                        .get("uuid")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&gen_uuid())
                        .to_string(),
                );
                tuic_password.set(
                    settings
                        .get("password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
                tuic_cc.set(
                    settings
                        .get("congestion_control")
                        .and_then(|v| v.as_str())
                        .unwrap_or("cubic")
                        .to_string(),
                );
            }
            _ => {}
        }
        if let Some(tls_obj) = tls.as_object() {
            new_tls_settings.set(serde_json::to_string_pretty(tls_obj).unwrap_or_default());
        } else {
            new_tls_settings.set(String::new());
        }
    };

    let build_settings = move || {
        let protocol_type = new_protocol_type.read().clone();
        match protocol_type.as_str() {
            "vless_reality" => json!({
                "uuid": vless_uuid.read().clone(),
                "flow": vless_flow.read().clone(),
                "reality_dest": reality_dest.read().clone(),
                "reality_server_names": reality_server_names.read().clone(),
                "reality_private_key": reality_private_key.read().clone(),
                "reality_public_key": reality_public_key.read().clone(),
                "reality_short_id": reality_short_id.read().clone(),
            }),
            "vless_vision" => json!({
                "uuid": vless_uuid.read().clone(),
                "flow": vless_flow.read().clone(),
            }),
            "vless_xhttp" => json!({
                "uuid": vless_uuid.read().clone(),
                "xhttp_path": xhttp_path.read().clone(),
                "xhttp_host": xhttp_host.read().clone(),
                "xhttp_mode": xhttp_mode.read().clone(),
            }),
            "vmess" => json!({
                "uuid": vmess_uuid.read().clone(),
                "alterId": vmess_alter_id.read().parse::<u64>().unwrap_or(0),
            }),
            "trojan" => json!({
                "password": trojan_password.read().clone(),
            }),
            "shadowsocks2022" => json!({
                "method": ss_method.read().clone(),
                "password": ss_password.read().clone(),
            }),
            "hysteria2" => json!({
                "password": h2_password.read().clone(),
                "obfs_type": h2_obfs_type.read().clone(),
                "obfs_password": h2_obfs_password.read().clone(),
                "up_mbps": h2_up_mbps.read().parse::<u64>().unwrap_or(100),
                "down_mbps": h2_down_mbps.read().parse::<u64>().unwrap_or(100),
            }),
            "anytls" => json!({
                "password": anytls_password.read().clone(),
                "masquerade": anytls_masquerade.read().clone(),
            }),
            "tuic" | "tuic_v5" => json!({
                "uuid": tuic_uuid.read().clone(),
                "password": tuic_password.read().clone(),
                "congestion_control": tuic_cc.read().clone(),
            }),
            _ => json!({}),
        }
    };

    let validate = move || -> Result<(u64, Option<Value>), String> {
        let port = new_listen_port
            .read()
            .parse::<u64>()
            .map_err(|_| "Port must be a number".to_string())?;
        if port == 0 || port > 65535 {
            return Err("Port must be between 1 and 65535".to_string());
        }
        let tls = parse_tls_settings(&new_tls_settings.read())?;
        Ok((port, tls))
    };

    let protocol_specific_form: Element = match pt.as_str() {
        "vless_reality" => rsx! {
            FormInput { label: t!("protocols-uuid").to_string(), value: vless_uuid, placeholder: Some("auto-generated".to_string()), input_type: None, error: None }
            FormSelect {
                label: t!("protocols-flow").to_string(),
                value: vless_flow,
                options: vec![
                    ("xtls-rprx-vision".to_string(), "xtls-rprx-vision".to_string()),
                    ("xtls-rprx-vision-udp443".to_string(), "xtls-rprx-vision-udp443".to_string()),
                ],
                error: None,
            }
            FormInput { label: t!("protocols-dest").to_string(), value: reality_dest, placeholder: Some("www.cloudflare.com:443".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-server-names").to_string(), value: reality_server_names, placeholder: Some("cf.com,www.cf.com".to_string()), input_type: None, error: None }
            div { class: "form-group",
                button {
                    onclick: move |_| {
                        spawn(async move {
                            if let Ok(resp) = api::generate_reality_keys().await {
                                if let Some(data) = resp.get("data") {
                                    if let Some(pk) = data.get("private_key").and_then(|v| v.as_str()) {
                                        reality_private_key.set(pk.to_string());
                                    }
                                    if let Some(pubk) = data.get("public_key").and_then(|v| v.as_str()) {
                                        reality_public_key.set(pubk.to_string());
                                    }
                                    if let Some(sid) = data.get("short_id").and_then(|v| v.as_str()) {
                                        reality_short_id.set(sid.to_string());
                                    }
                                }
                            }
                        });
                    },
                    {t!("protocols-generate-keys")}
                }
            }
            FormInput { label: t!("protocols-private-key").to_string(), value: reality_private_key, placeholder: Some("base64 private key".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-public-key").to_string(), value: reality_public_key, placeholder: Some("base64 public key".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-short-id").to_string(), value: reality_short_id, placeholder: Some("8 hex chars".to_string()), input_type: None, error: None }
        },
        "vless_xhttp" => rsx! {
            FormInput { label: t!("protocols-uuid").to_string(), value: vless_uuid, placeholder: Some("auto-generated".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-path").to_string(), value: xhttp_path, placeholder: Some("/xhttp".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-host").to_string(), value: xhttp_host, placeholder: Some("host.example.com".to_string()), input_type: None, error: None }
            FormSelect {
                label: t!("protocols-mode").to_string(),
                value: xhttp_mode,
                options: vec![
                    ("auto".to_string(), "auto".to_string()),
                    ("packet-up".to_string(), "packet-up".to_string()),
                    ("stream-up".to_string(), "stream-up".to_string()),
                ],
                error: None,
            }
        },
        "hysteria2" => rsx! {
            FormInput { label: t!("protocols-password").to_string(), value: h2_password, placeholder: Some("password".to_string()), input_type: None, error: None }
            FormSelect {
                label: t!("protocols-obfs-type").to_string(),
                value: h2_obfs_type,
                options: vec![
                    ("none".to_string(), "none".to_string()),
                    ("salamander".to_string(), "salamander".to_string()),
                ],
                error: None,
            }
            FormInput { label: t!("protocols-obfs-password").to_string(), value: h2_obfs_password, placeholder: Some("".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-up-mbps").to_string(), value: h2_up_mbps, placeholder: Some("100".to_string()), input_type: Some("number".to_string()), error: None }
            FormInput { label: t!("protocols-down-mbps").to_string(), value: h2_down_mbps, placeholder: Some("100".to_string()), input_type: Some("number".to_string()), error: None }
        },
        "vless_vision" => rsx! {
            FormInput { label: t!("protocols-uuid").to_string(), value: vless_uuid, placeholder: Some("auto-generated".to_string()), input_type: None, error: None }
            FormSelect {
                label: t!("protocols-flow").to_string(),
                value: vless_flow,
                options: vec![
                    ("xtls-rprx-vision".to_string(), "xtls-rprx-vision".to_string()),
                    ("xtls-rprx-vision-udp443".to_string(), "xtls-rprx-vision-udp443".to_string()),
                ],
                error: None,
            }
        },
        "vmess" => rsx! {
            FormInput { label: t!("protocols-uuid").to_string(), value: vmess_uuid, placeholder: Some("auto-generated".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-alter-id").to_string(), value: vmess_alter_id, placeholder: Some("0".to_string()), input_type: Some("number".to_string()), error: None }
        },
        "trojan" => rsx! {
            FormInput { label: t!("protocols-password").to_string(), value: trojan_password, placeholder: Some("password".to_string()), input_type: None, error: None }
        },
        "shadowsocks2022" => rsx! {
            FormSelect {
                label: t!("protocols-method").to_string(),
                value: ss_method,
                options: vec![
                    ("2022-blake3-aes-128-gcm".to_string(), "2022-blake3-aes-128-gcm".to_string()),
                    ("2022-blake3-aes-256-gcm".to_string(), "2022-blake3-aes-256-gcm".to_string()),
                    ("2022-blake3-chacha20-poly1305".to_string(), "2022-blake3-chacha20-poly1305".to_string()),
                ],
                error: None,
            }
            FormInput { label: t!("protocols-password").to_string(), value: ss_password, placeholder: Some("password".to_string()), input_type: None, error: None }
        },
        "anytls" => rsx! {
            FormInput { label: t!("protocols-password").to_string(), value: anytls_password, placeholder: Some("password".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-masquerade").to_string(), value: anytls_masquerade, placeholder: Some("https://example.com".to_string()), input_type: None, error: None }
        },
        "tuic" | "tuic_v5" => rsx! {
            FormInput { label: t!("protocols-uuid").to_string(), value: tuic_uuid, placeholder: Some("auto-generated".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-password").to_string(), value: tuic_password, placeholder: Some("password".to_string()), input_type: None, error: None }
            FormSelect {
                label: t!("protocols-congestion-control").to_string(),
                value: tuic_cc,
                options: vec![
                    ("cubic".to_string(), "cubic".to_string()),
                    ("bbr".to_string(), "bbr".to_string()),
                    ("none".to_string(), "none".to_string()),
                ],
                error: None,
            }
        },
        _ => rsx! {},
    };

    let core_options: Vec<(String, String)> = match pt.as_str() {
        "vless_reality" | "vless_vision" | "vmess" | "trojan" | "shadowsocks2022" => vec![
            ("xray".to_string(), "Xray".to_string()),
            ("sing-box".to_string(), "sing-box".to_string()),
        ],
        "vless_xhttp" => vec![("xray".to_string(), "Xray".to_string())],
        "hysteria2" | "anytls" | "tuic" | "tuic_v5" => {
            vec![("sing-box".to_string(), "sing-box".to_string())]
        }
        _ => vec![("sing-box".to_string(), "sing-box".to_string())],
    };

    let protocol_type_options = vec![
        (
            "vless_reality".to_string(),
            t!("protocols-protocol-vless-reality").to_string(),
        ),
        (
            "vless_vision".to_string(),
            t!("protocols-protocol-vless-vision").to_string(),
        ),
        (
            "vless_xhttp".to_string(),
            t!("protocols-protocol-vless-xhttp").to_string(),
        ),
        (
            "vmess".to_string(),
            t!("protocols-protocol-vmess").to_string(),
        ),
        (
            "trojan".to_string(),
            t!("protocols-protocol-trojan").to_string(),
        ),
        (
            "shadowsocks2022".to_string(),
            t!("protocols-protocol-shadowsocks2022").to_string(),
        ),
        (
            "hysteria2".to_string(),
            t!("protocols-protocol-hysteria2").to_string(),
        ),
        (
            "anytls".to_string(),
            t!("protocols-protocol-anytls").to_string(),
        ),
        (
            "tuic_v5".to_string(),
            t!("protocols-protocol-tuic-v5").to_string(),
        ),
    ];

    let modal_title = if *is_edit.read() {
        t!("protocols-edit-title").to_string()
    } else {
        t!("protocols-create-title").to_string()
    };
    let confirm_text = if *is_edit.read() {
        t!("common-update").to_string()
    } else {
        t!("common-create").to_string()
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("protocols-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        error.set(None);
                        is_edit.set(false);
                        show_modal.set(true);
                    },
                    {t!("protocols-create")}
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if protocols_data.is_empty() {
                div { class: "empty-state", p { "No protocol configs found." } }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("common-name")} }
                            th { {t!("protocols-type")} }
                            th { {t!("protocols-core")} }
                            th { {t!("protocols-listen")} }
                            th { {t!("protocols-port")} }
                            th { {t!("protocols-key")} }
                            th { {t!("common-actions")} }
                        }
                    }
                    tbody {
                        for p in protocols_data.iter() {
                            {
                                let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let protocol_type = p.get("protocol_type").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let core_type = p.get("core_type").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let listen_address = p.get("listen_address").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let listen_port = p.get("listen_port").and_then(|v| v.as_i64()).unwrap_or(0);
                                let settings = p.get("settings").cloned().unwrap_or(json!({}));
                                let tls_settings = p.get("tls_settings").cloned().unwrap_or(json!({}));
                                let summary = protocol_settings_summary(&protocol_type, &settings);
                                let did = id.clone();
                                let edit_id_clone = id.clone();
                                let edit_name = name.clone();
                                let edit_protocol_type = protocol_type.clone();
                                let edit_core_type = core_type.clone();
                                let edit_listen_address = listen_address.clone();
                                let edit_listen_port = listen_port;
                                let edit_settings = settings.clone();
                                let edit_tls = tls_settings.clone();
                                rsx! {
                                    tr {
                                        td { "{name}" }
                                        td { "{protocol_type}" }
                                        td { "{core_type}" }
                                        td { "{listen_address}" }
                                        td { "{listen_port}" }
                                        td { class: "mono", "{summary}" }
                                        td {
                                            button {
                                                onclick: move |_| {
                                                    let pt = edit_protocol_type.clone();
                                                    let settings = edit_settings.clone();
                                                    let tls = edit_tls.clone();
                                                    new_name.set(edit_name.clone());
                                                    new_protocol_type.set(pt.clone());
                                                    new_core_type.set(edit_core_type.clone());
                                                    new_listen_address.set(edit_listen_address.clone());
                                                    new_listen_port.set(edit_listen_port.to_string());
                                                    load_settings(&pt, &settings, &tls);
                                                    edit_id.set(edit_id_clone.clone());
                                                    is_edit.set(true);
                                                    error.set(None);
                                                    show_modal.set(true);
                                                },
                                                {t!("common-edit")}
                                            }
                                            button {
                                                class: "danger",
                                                onclick: move |_| {
                                                    delete_id.set(did.clone());
                                                    show_delete.set(true);
                                                },
                                                {t!("common-delete")}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Pagination {
                page: current_page,
                per_page: current_per_page,
                total,
                on_page_change: move |p: u64| page.set(p),
                on_per_page_change: move |pp: u64| {
                    per_page.set(pp);
                    page.set(1);
                },
            }
        }

        Modal {
            title: modal_title,
            show: show_modal,
            on_confirm: move |_| {
                let name = new_name.read().clone();
                if name.is_empty() {
                    error.set(Some(t!("common-required-field").to_string()));
                    return;
                }
                let protocol_type = new_protocol_type.read().clone();
                let core_type = new_core_type.read().clone();
                let listen_address = new_listen_address.read().clone();
                let (port, tls_settings) = match validate() {
                    Ok(v) => v,
                    Err(e) => { error.set(Some(e)); return; }
                };
                let settings = build_settings();
                if *is_edit.read() {
                    let id = edit_id.read().clone();
                    let mut payload = json!({
                        "name": name,
                        "protocol_type": protocol_type,
                        "core_type": core_type,
                        "listen_address": listen_address,
                        "listen_port": port,
                        "settings": settings,
                    });
                    if let Some(tls) = tls_settings {
                        payload["tls_settings"] = tls;
                    }
                    spawn(async move {
                        match api::update_protocol(&id, payload).await {
                            Ok(_) => {
                                reset_form();
                                is_edit.set(false);
                                show_modal.set(false);
                                protocols.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                } else {
                    spawn(async move {
                        match api::create_protocol(
                            &name, &protocol_type, &core_type, &listen_address, port, settings, tls_settings
                        ).await {
                            Ok(_) => {
                                reset_form();
                                show_modal.set(false);
                                protocols.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                }
            },
            confirm_text: Some(confirm_text),
            FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("vless-443".to_string()), input_type: None, error: None }
            FormSelect {
                label: t!("protocols-type").to_string(),
                value: new_protocol_type,
                options: protocol_type_options,
                error: None,
            }
            FormSelect {
                label: t!("protocols-core").to_string(),
                value: new_core_type,
                options: core_options,
                error: None,
            }
            FormInput { label: t!("protocols-listen").to_string(), value: new_listen_address, placeholder: Some("0.0.0.0".to_string()), input_type: None, error: None }
            FormInput { label: t!("protocols-port").to_string(), value: new_listen_port, placeholder: Some("443".to_string()), input_type: Some("number".to_string()), error: None }
            {protocol_specific_form}
            FormTextarea { label: t!("protocols-tls-settings").to_string(), value: new_tls_settings, placeholder: Some("{\"serverName\": \"example.com\"}".to_string()), rows: Some(3), error: None }
        }

        ConfirmDialog {
            title: t!("protocols-delete-title").to_string(),
            message: t!("protocols-delete-confirm").to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_protocol(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            protocols.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

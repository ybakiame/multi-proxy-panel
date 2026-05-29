use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{json, Value};

use crate::api;
use crate::components::{FormInput, FormSelect, Modal};

fn gen_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn protocol_settings_summary(pt: &str, settings: &Value) -> String {
    match pt {
        "vless_reality" | "vless_xhttp" => {
            settings.get("uuid").and_then(|v| v.as_str()).unwrap_or("-").to_string()
        }
        "hysteria2" => {
            settings.get("password").and_then(|v| v.as_str()).unwrap_or("-").to_string()
        }
        "anytls" => {
            settings.get("password").and_then(|v| v.as_str()).unwrap_or("-").to_string()
        }
        "tuic" => {
            let uuid = settings.get("uuid").and_then(|v| v.as_str()).unwrap_or("-");
            format!("{}", uuid)
        }
        _ => "-".to_string(),
    }
}

#[component]
pub fn Protocols() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut protocols = use_resource(move || async move {
        api::get_protocols(*page.read(), *per_page.read()).await.unwrap_or_default()
    });

    let mut show_modal = use_signal(|| false);
    let mut is_edit = use_signal(|| false);
    let mut edit_id = use_signal(|| String::new());

    // Common fields
    let mut new_name = use_signal(|| String::new());
    let mut new_protocol_type = use_signal(|| "vless_reality".to_string());
    let mut new_core_type = use_signal(|| "xray".to_string());
    let mut new_listen_address = use_signal(|| "0.0.0.0".to_string());
    let mut new_listen_port = use_signal(|| "443".to_string());

    // VLESS fields
    let mut vless_uuid = use_signal(|| gen_uuid());
    let mut vless_flow = use_signal(|| "xtls-rprx-vision".to_string());

    // VLESS + REALITY fields
    let mut reality_dest = use_signal(|| "www.cloudflare.com:443".to_string());
    let mut reality_server_names = use_signal(|| String::new());
    let mut reality_private_key = use_signal(|| String::new());
    let mut reality_public_key = use_signal(|| String::new());
    let mut reality_short_id = use_signal(|| String::new());

    // VLESS + XHTTP fields
    let mut xhttp_path = use_signal(|| "/xhttp".to_string());
    let mut xhttp_host = use_signal(|| String::new());
    let mut xhttp_mode = use_signal(|| "auto".to_string());

    // Hysteria2 fields
    let mut h2_password = use_signal(|| String::new());
    let mut h2_obfs_type = use_signal(|| "none".to_string());
    let mut h2_obfs_password = use_signal(|| String::new());
    let mut h2_up_mbps = use_signal(|| "100".to_string());
    let mut h2_down_mbps = use_signal(|| "100".to_string());

    // AnyTLS fields
    let mut anytls_password = use_signal(|| String::new());

    // TUIC fields
    let mut tuic_uuid = use_signal(|| gen_uuid());
    let mut tuic_password = use_signal(|| String::new());
    let mut tuic_cc = use_signal(|| "cubic".to_string());

    let protocols_data = protocols.read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = protocols.read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let current_per_page = *per_page.read();
    let total_pages = if total == 0 { 1 } else { (total + current_per_page - 1) / current_per_page };
    let current_page = *page.read();

    let pt = new_protocol_type.read().clone();

    let mut reset_form = move || {
        new_name.set(String::new());
        new_protocol_type.set("vless_reality".to_string());
        new_core_type.set("xray".to_string());
        new_listen_address.set("0.0.0.0".to_string());
        new_listen_port.set("443".to_string());
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
        h2_password.set(String::new());
        h2_obfs_type.set("none".to_string());
        h2_obfs_password.set(String::new());
        h2_up_mbps.set("100".to_string());
        h2_down_mbps.set("100".to_string());
        anytls_password.set(String::new());
        tuic_uuid.set(gen_uuid());
        tuic_password.set(String::new());
        tuic_cc.set("cubic".to_string());
    };

    let mut load_settings = move |protocol_type: &str, settings: &Value| {
        match protocol_type {
            "vless_reality" => {
                vless_uuid.set(settings.get("uuid").and_then(|v| v.as_str()).unwrap_or(&gen_uuid()).to_string());
                vless_flow.set(settings.get("flow").and_then(|v| v.as_str()).unwrap_or("xtls-rprx-vision").to_string());
                reality_dest.set(settings.get("dest").and_then(|v| v.as_str()).unwrap_or("www.cloudflare.com:443").to_string());
                reality_server_names.set(settings.get("server_names").and_then(|v| v.as_str()).unwrap_or("").to_string());
                reality_private_key.set(settings.get("private_key").and_then(|v| v.as_str()).unwrap_or("").to_string());
                reality_public_key.set(settings.get("public_key").and_then(|v| v.as_str()).unwrap_or("").to_string());
                reality_short_id.set(settings.get("short_id").and_then(|v| v.as_str()).unwrap_or("").to_string());
            }
            "vless_xhttp" => {
                vless_uuid.set(settings.get("uuid").and_then(|v| v.as_str()).unwrap_or(&gen_uuid()).to_string());
                xhttp_path.set(settings.get("path").and_then(|v| v.as_str()).unwrap_or("/xhttp").to_string());
                xhttp_host.set(settings.get("host").and_then(|v| v.as_str()).unwrap_or("").to_string());
                xhttp_mode.set(settings.get("mode").and_then(|v| v.as_str()).unwrap_or("auto").to_string());
            }
            "hysteria2" => {
                h2_password.set(settings.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string());
                h2_obfs_type.set(settings.get("obfs_type").and_then(|v| v.as_str()).unwrap_or("none").to_string());
                h2_obfs_password.set(settings.get("obfs_password").and_then(|v| v.as_str()).unwrap_or("").to_string());
                h2_up_mbps.set(settings.get("up_mbps").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_else(|| "100".to_string()));
                h2_down_mbps.set(settings.get("down_mbps").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_else(|| "100".to_string()));
            }
            "anytls" => {
                anytls_password.set(settings.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string());
            }
            "tuic" => {
                tuic_uuid.set(settings.get("uuid").and_then(|v| v.as_str()).unwrap_or(&gen_uuid()).to_string());
                tuic_password.set(settings.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string());
                tuic_cc.set(settings.get("congestion_control").and_then(|v| v.as_str()).unwrap_or("cubic").to_string());
            }
            _ => {}
        }
    };

    let build_settings = move || {
        let protocol_type = new_protocol_type.read().clone();
        match protocol_type.as_str() {
            "vless_reality" => json!({
                "uuid": vless_uuid.read().clone(),
                "flow": vless_flow.read().clone(),
                "dest": reality_dest.read().clone(),
                "server_names": reality_server_names.read().clone(),
                "private_key": reality_private_key.read().clone(),
                "public_key": reality_public_key.read().clone(),
                "short_id": reality_short_id.read().clone(),
            }),
            "vless_xhttp" => json!({
                "uuid": vless_uuid.read().clone(),
                "path": xhttp_path.read().clone(),
                "host": xhttp_host.read().clone(),
                "mode": xhttp_mode.read().clone(),
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
            }),
            "tuic" => json!({
                "uuid": tuic_uuid.read().clone(),
                "password": tuic_password.read().clone(),
                "congestion_control": tuic_cc.read().clone(),
            }),
            _ => json!({}),
        }
    };

    let protocol_specific_form: Element = match pt.as_str() {
        "vless_reality" => rsx! {
            FormInput { label: "UUID".to_string(), value: vless_uuid, placeholder: Some("auto-generated".to_string()), input_type: None }
            FormSelect {
                label: "Flow".to_string(),
                value: vless_flow,
                options: vec![
                    ("xtls-rprx-vision".to_string(), "xtls-rprx-vision".to_string()),
                    ("xtls-rprx-vision-udp443".to_string(), "xtls-rprx-vision-udp443".to_string()),
                ],
            }
            FormInput { label: "Reality Dest".to_string(), value: reality_dest, placeholder: Some("www.cloudflare.com:443".to_string()), input_type: None }
            FormInput { label: "Server Names (comma separated)".to_string(), value: reality_server_names, placeholder: Some("cf.com,www.cf.com".to_string()), input_type: None }
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
                    "🔑 Generate Keys"
                }
            }
            FormInput { label: "Private Key".to_string(), value: reality_private_key, placeholder: Some("base64 private key".to_string()), input_type: None }
            FormInput { label: "Public Key".to_string(), value: reality_public_key, placeholder: Some("base64 public key".to_string()), input_type: None }
            FormInput { label: "Short ID".to_string(), value: reality_short_id, placeholder: Some("8 hex chars".to_string()), input_type: None }
        },
        "vless_xhttp" => rsx! {
            FormInput { label: "UUID".to_string(), value: vless_uuid, placeholder: Some("auto-generated".to_string()), input_type: None }
            FormInput { label: "Path".to_string(), value: xhttp_path, placeholder: Some("/xhttp".to_string()), input_type: None }
            FormInput { label: "Host".to_string(), value: xhttp_host, placeholder: Some("host.example.com".to_string()), input_type: None }
            FormSelect {
                label: "Mode".to_string(),
                value: xhttp_mode,
                options: vec![
                    ("auto".to_string(), "auto".to_string()),
                    ("packet-up".to_string(), "packet-up".to_string()),
                    ("stream-up".to_string(), "stream-up".to_string()),
                ],
            }
        },
        "hysteria2" => rsx! {
            FormInput { label: "Password".to_string(), value: h2_password, placeholder: Some("password".to_string()), input_type: None }
            FormSelect {
                label: "Obfs Type".to_string(),
                value: h2_obfs_type,
                options: vec![
                    ("none".to_string(), "none".to_string()),
                    ("salamander".to_string(), "salamander".to_string()),
                ],
            }
            FormInput { label: "Obfs Password".to_string(), value: h2_obfs_password, placeholder: Some("".to_string()), input_type: None }
            FormInput { label: "Up Mbps".to_string(), value: h2_up_mbps, placeholder: Some("100".to_string()), input_type: Some("number".to_string()) }
            FormInput { label: "Down Mbps".to_string(), value: h2_down_mbps, placeholder: Some("100".to_string()), input_type: Some("number".to_string()) }
        },
        "anytls" => rsx! {
            FormInput { label: "Password".to_string(), value: anytls_password, placeholder: Some("password".to_string()), input_type: None }
        },
        "tuic" => rsx! {
            FormInput { label: "UUID".to_string(), value: tuic_uuid, placeholder: Some("auto-generated".to_string()), input_type: None }
            FormInput { label: "Password".to_string(), value: tuic_password, placeholder: Some("password".to_string()), input_type: None }
            FormSelect {
                label: "Congestion Control".to_string(),
                value: tuic_cc,
                options: vec![
                    ("cubic".to_string(), "cubic".to_string()),
                    ("bbr".to_string(), "bbr".to_string()),
                    ("none".to_string(), "none".to_string()),
                ],
            }
        },
        _ => rsx! {},
    };

    let core_options: Vec<(String, String)> = match pt.as_str() {
        "vless_reality" | "hysteria2" => vec![
            ("xray".to_string(), "Xray".to_string()),
            ("sing-box".to_string(), "sing-box".to_string()),
        ],
        "vless_xhttp" => vec![
            ("xray".to_string(), "Xray".to_string()),
        ],
        _ => vec![
            ("sing-box".to_string(), "sing-box".to_string()),
        ],
    };

    let modal_title = if *is_edit.read() { t!("protocols-edit-title").to_string() } else { t!("protocols-create-title").to_string() };
    let confirm_text = if *is_edit.read() { t!("common-update").to_string() } else { t!("common-create").to_string() };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("protocols-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        is_edit.set(false);
                        show_modal.set(true);
                    },
                    {t!("protocols-create")}
                }
            }

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
                            let summary = protocol_settings_summary(&protocol_type, &settings);
                            let delete_id = id.clone();
                            let edit_id_clone = id.clone();
                            let edit_name = name.clone();
                            let edit_protocol_type = protocol_type.clone();
                            let edit_core_type = core_type.clone();
                            let edit_listen_address = listen_address.clone();
                            let edit_listen_port = listen_port;
                            let edit_settings = settings.clone();
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
                                                new_name.set(edit_name.clone());
                                                new_protocol_type.set(pt.clone());
                                                new_core_type.set(edit_core_type.clone());
                                                new_listen_address.set(edit_listen_address.clone());
                                                new_listen_port.set(edit_listen_port.to_string());
                                                load_settings(&pt, &settings);
                                                edit_id.set(edit_id_clone.clone());
                                                is_edit.set(true);
                                                show_modal.set(true);
                                            },
                                            {t!("common-edit")}
                                        }
                                        button {
                                            class: "danger",
                                            onclick: move |_| {
                                                let id = delete_id.clone();
                                                spawn(async move {
                                                    let _ = api::delete_protocol(&id).await;
                                                    protocols.restart();
                                                });
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

            div { class: "pagination",
                button {
                    disabled: current_page <= 1,
                    onclick: move |_| {
                        if current_page > 1 {
                            page.set(current_page - 1);
                        }
                    },
                    {t!("protocols-prev")}
                }
                span { {t!("protocols-page", current: current_page, total: total_pages, count: total)} }
                button {
                    disabled: current_page >= total_pages,
                    onclick: move |_| {
                        if current_page < total_pages {
                            page.set(current_page + 1);
                        }
                    },
                    {t!("protocols-next")}
                }
                select {
                    value: "{current_per_page}",
                    onchange: move |e| {
                        let val = e.value().parse::<u64>().unwrap_or(20);
                        per_page.set(val);
                        page.set(1);
                    },
                    option { value: "10", "10 / page" }
                    option { value: "20", selected: current_per_page == 20, "20 / page" }
                    option { value: "50", selected: current_per_page == 50, "50 / page" }
                }
            }
        }

        Modal {
            title: modal_title,
            show: show_modal,
            on_confirm: move |_| {
                let name = new_name.read().clone();
                let protocol_type = new_protocol_type.read().clone();
                let core_type = new_core_type.read().clone();
                let listen_address = new_listen_address.read().clone();
                let port = new_listen_port.read().parse::<u64>().unwrap_or(443);
                if !name.is_empty() {
                    let settings = build_settings();
                    if *is_edit.read() {
                        let id = edit_id.read().clone();
                        let payload = json!({
                            "name": name,
                            "protocol_type": protocol_type,
                            "core_type": core_type,
                            "listen_address": listen_address,
                            "listen_port": port,
                            "settings": settings,
                        });
                        spawn(async move {
                            let _ = api::update_protocol(&id, payload).await;
                            reset_form();
                            show_modal.set(false);
                            protocols.restart();
                        });
                    } else {
                        spawn(async move {
                            let _ = api::create_protocol(&name, &protocol_type, &core_type, &listen_address, port, settings).await;
                            reset_form();
                            show_modal.set(false);
                            protocols.restart();
                        });
                    }
                }
            },
            confirm_text: Some(confirm_text),
            FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("vless-443".to_string()), input_type: None }
            FormSelect {
                label: t!("protocols-type").to_string(),
                value: new_protocol_type,
                options: vec![
                    ("vless_reality".to_string(), "VLESS + REALITY".to_string()),
                    ("vless_xhttp".to_string(), "VLESS + XHTTP".to_string()),
                    ("hysteria2".to_string(), "Hysteria2".to_string()),
                    ("anytls".to_string(), "AnyTLS".to_string()),
                    ("tuic".to_string(), "TUIC".to_string()),
                ],
            }
            FormSelect {
                label: t!("protocols-core").to_string(),
                value: new_core_type,
                options: core_options,
            }
            FormInput { label: t!("protocols-listen").to_string(), value: new_listen_address, placeholder: Some("0.0.0.0".to_string()), input_type: None }
            FormInput { label: t!("protocols-port").to_string(), value: new_listen_port, placeholder: Some("443".to_string()), input_type: Some("number".to_string()) }
            {protocol_specific_form}
        }
    }
}

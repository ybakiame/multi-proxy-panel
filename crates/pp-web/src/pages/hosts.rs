use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{Alert, ConfirmDialog, FormInput, Modal, Pagination, StatusBadge};

#[component]
pub fn Hosts() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut hosts = use_resource(move || async move {
        api::get_hosts_paginated(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });

    let mut show_modal = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut is_edit = use_signal(|| false);
    let mut edit_id = use_signal(String::new);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut host_protocol_config_id = use_signal(String::new);
    let mut host_node_id = use_signal(String::new);
    let mut host_remark = use_signal(String::new);
    let mut host_address = use_signal(String::new);
    let mut host_port = use_signal(|| "443".to_string());
    let mut host_sni = use_signal(String::new);
    let mut host_host = use_signal(String::new);
    let mut host_path = use_signal(String::new);
    let mut host_is_active = use_signal(|| true);

    let hosts_data = hosts
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = hosts
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut reset_form = move || {
        host_protocol_config_id.set(String::new());
        host_node_id.set(String::new());
        host_remark.set(String::new());
        host_address.set(String::new());
        host_port.set("443".to_string());
        host_sni.set(String::new());
        host_host.set(String::new());
        host_path.set(String::new());
        host_is_active.set(true);
    };

    let mut load_form = move |h: &Value| {
        host_protocol_config_id.set(h.get("protocol_config_id").and_then(|v| v.as_str()).unwrap_or("").to_string());
        host_node_id.set(h.get("node_id").and_then(|v| v.as_str()).unwrap_or("").to_string());
        host_remark.set(h.get("remark").and_then(|v| v.as_str()).unwrap_or("").to_string());
        host_address.set(h.get("address").and_then(|v| v.as_str()).unwrap_or("").to_string());
        host_port.set(h.get("port").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_else(|| "443".to_string()));
        host_sni.set(h.get("sni").and_then(|v| v.as_str()).unwrap_or("").to_string());
        host_host.set(h.get("host").and_then(|v| v.as_str()).unwrap_or("").to_string());
        host_path.set(h.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string());
        host_is_active.set(h.get("is_active").and_then(|v| v.as_bool()).unwrap_or(true));
    };

    let modal_title = if *is_edit.read() {
        t!("hosts-edit-title").to_string()
    } else {
        t!("hosts-create-title").to_string()
    };
    let confirm_text = if *is_edit.read() {
        t!("common-update").to_string()
    } else {
        t!("common-create").to_string()
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("hosts-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        error.set(None);
                        is_edit.set(false);
                        show_modal.set(true);
                    },
                    {t!("hosts-create")}
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if hosts_data.is_empty() {
                div { class: "empty-state", p { "No hosts found." } }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("hosts-remark")} }
                            th { {t!("hosts-address")} }
                            th { {t!("hosts-port")} }
                            th { {t!("hosts-sni")} }
                            th { {t!("hosts-host")} }
                            th { {t!("hosts-path")} }
                            th { {t!("common-active")} }
                            th { {t!("hosts-protocol-config")} }
                            th { {t!("hosts-node")} }
                            th { {t!("common-actions")} }
                        }
                    }
                    tbody {
                        for h in hosts_data.iter() {
                            {
                                let id = h.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let remark = h.get("remark").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let address = h.get("address").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let port = h.get("port").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_else(|| "-".to_string());
                                let sni = h.get("sni").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let host_val = h.get("host").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let path = h.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let active = h.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
                                let status = if active { "active" } else { "inactive" };
                                let protocol_config_id = h.get("protocol_config_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let node_id = h.get("node_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let did = id.clone();
                                let edit_id_clone = id.clone();
                                let edit_data = h.clone();
                                rsx! {
                                    tr {
                                        td { "{remark}" }
                                        td { "{address}" }
                                        td { "{port}" }
                                        td {
                                            if sni.is_empty() { "-" } else { "{sni}" }
                                        }
                                        td {
                                            if host_val.is_empty() { "-" } else { "{host_val}" }
                                        }
                                        td {
                                            if path.is_empty() { "-" } else { "{path}" }
                                        }
                                        td {
                                            StatusBadge { status: status.to_string() }
                                        }
                                        td { class: "mono", "{protocol_config_id}" }
                                        td { class: "mono", "{node_id}" }
                                        td {
                                            button {
                                                onclick: move |_| {
                                                    load_form(&edit_data);
                                                    edit_id.set(edit_id_clone.clone());
                                                    error.set(None);
                                                    is_edit.set(true);
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
                page: *page.read(),
                per_page: *per_page.read(),
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
                let protocol_config_id = host_protocol_config_id.read().clone();
                let node_id = host_node_id.read().clone();
                let remark = host_remark.read().clone();
                let address = host_address.read().clone();
                if protocol_config_id.is_empty() || node_id.is_empty() || remark.is_empty() || address.is_empty() {
                    error.set(Some(t!("common-required-field").to_string()));
                    return;
                }
                let port = host_port.read().parse::<i32>().unwrap_or(443);
                let sni_opt = {
                    let s = host_sni.read().clone();
                    if s.is_empty() { None } else { Some(s) }
                };
                let host_opt = {
                    let s = host_host.read().clone();
                    if s.is_empty() { None } else { Some(s) }
                };
                let path_opt = {
                    let s = host_path.read().clone();
                    if s.is_empty() { None } else { Some(s) }
                };
                let is_active = *host_is_active.read();

                if *is_edit.read() {
                    let id = edit_id.read().clone();
                    spawn(async move {
                        let mut payload = json!({
                            "remark": remark,
                            "address": address,
                            "port": port,
                            "is_active": is_active,
                        });
                        if let Some(s) = sni_opt { payload["sni"] = json!(s); }
                        if let Some(h) = host_opt { payload["host"] = json!(h); }
                        if let Some(p) = path_opt { payload["path"] = json!(p); }
                        match api::update_host(&id, payload).await {
                            Ok(_) => {
                                reset_form();
                                is_edit.set(false);
                                show_modal.set(false);
                                hosts.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                } else {
                    spawn(async move {
                        let mut payload = json!({
                            "protocol_config_id": protocol_config_id,
                            "node_id": node_id,
                            "remark": remark,
                            "address": address,
                            "port": port,
                            "is_active": is_active,
                        });
                        if let Some(s) = sni_opt { payload["sni"] = json!(s); }
                        if let Some(h) = host_opt { payload["host"] = json!(h); }
                        if let Some(p) = path_opt { payload["path"] = json!(p); }
                        match api::create_host(payload).await {
                            Ok(_) => {
                                reset_form();
                                show_modal.set(false);
                                hosts.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                }
            },
            confirm_text: Some(confirm_text),
            FormInput { label: t!("hosts-protocol-config").to_string(), value: host_protocol_config_id, placeholder: Some("00000000-0000-0000-0000-000000000000".to_string()), input_type: None, error: None }
            FormInput { label: t!("hosts-node").to_string(), value: host_node_id, placeholder: Some("00000000-0000-0000-0000-000000000000".to_string()), input_type: None, error: None }
            FormInput { label: t!("hosts-remark").to_string(), value: host_remark, placeholder: Some("us-east-01".to_string()), input_type: None, error: None }
            FormInput { label: t!("hosts-address").to_string(), value: host_address, placeholder: Some("example.com".to_string()), input_type: None, error: None }
            FormInput { label: t!("hosts-port").to_string(), value: host_port, placeholder: Some("443".to_string()), input_type: Some("number".to_string()), error: None }
            FormInput { label: t!("hosts-sni").to_string(), value: host_sni, placeholder: Some("optional".to_string()), input_type: None, error: None }
            FormInput { label: t!("hosts-host").to_string(), value: host_host, placeholder: Some("optional".to_string()), input_type: None, error: None }
            FormInput { label: t!("hosts-path").to_string(), value: host_path, placeholder: Some("optional".to_string()), input_type: None, error: None }
            div { class: "form-group",
                label { class: "checkbox-label",
                    input {
                        r#type: "checkbox",
                        checked: "{host_is_active.read().clone()}",
                        onchange: move |e| host_is_active.set(e.checked()),
                    }
                    " "
                    {t!("common-active")}
                }
            }
        }

        ConfirmDialog {
            title: t!("hosts-delete-title").to_string(),
            message: t!("hosts-delete-confirm").to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_host(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            hosts.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}
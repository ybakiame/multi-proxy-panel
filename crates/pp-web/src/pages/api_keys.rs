use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{Alert, ConfirmDialog, FormInput, FormTextarea, Modal, Pagination};

#[component]
pub fn ApiKeys() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut keys = use_resource(move || async move {
        api::get_api_keys_paginated(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });

    let mut show_modal = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut new_name = use_signal(String::new);
    let mut new_scopes = use_signal(|| "[\"*\"]".to_string());
    let mut new_ip_allowlist = use_signal(|| "[]".to_string());
    let mut new_rate_limit = use_signal(String::new);
    let mut created_key = use_signal(|| None::<String>);

    let keys_data = keys
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = keys
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut reset_form = move || {
        new_name.set(String::new());
        new_scopes.set("[\"*\"]".to_string());
        new_ip_allowlist.set("[]".to_string());
        new_rate_limit.set(String::new());
        created_key.set(None);
    };

    let validate_json = |s: &str, label: &str| -> Result<Value, String> {
        serde_json::from_str(s).map_err(|e| format!("{} is not valid JSON: {}", label, e))
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("api-keys-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        error.set(None);
                        show_modal.set(true);
                    },
                    {t!("api-keys-create")}
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { {t!("common-name")} }
                        th { {t!("api-keys-scopes")} }
                        th { {t!("common-status")} }
                        th { {t!("common-actions")} }
                    }
                }
                tbody {
                    for k in keys_data.iter() {
                        {
                            let id = k.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let name = k.get("name").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let scopes = k.get("scopes").cloned().unwrap_or(json!([]));
                            let is_active = k.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
                            let status = if is_active { "active" } else { "inactive" };
                            let did = id.clone();
                            rsx! {
                                tr {
                                    td { "{name}" }
                                    td { class: "mono", "{scopes}" }
                                    td { "{status}" }
                                    td {
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
            title: t!("api-keys-create-title").to_string(),
            show: show_modal,
            on_confirm: move |_| {
                let name = new_name.read().clone();
                if name.is_empty() {
                    error.set(Some("Name is required".to_string()));
                    return;
                }
                let scopes = match validate_json(&new_scopes.read(), "Scopes") {
                    Ok(v) => v,
                    Err(e) => { error.set(Some(e)); return; }
                };
                let ip_allowlist = match validate_json(&new_ip_allowlist.read(), "IP Allowlist") {
                    Ok(v) => Some(v),
                    Err(e) => { error.set(Some(e)); return; }
                };
                let rate_limit = new_rate_limit.read().parse::<i32>().ok();
                spawn(async move {
                    match api::create_api_key(&name,
                        scopes,
                        ip_allowlist,
                        rate_limit,
                    ).await {
                        Ok(resp) => {
                            if let Some(key) = resp.get("data")
                                .and_then(|d| d.get("key"))
                                .and_then(|v| v.as_str())
                            {
                                created_key.set(Some(key.to_string()));
                            }
                            keys.restart();
                        }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            confirm_text: Some(t!("common-create").to_string()),
            FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("admin-cli".to_string()), input_type: None, error: None }
            FormTextarea { label: t!("api-keys-scopes").to_string(), value: new_scopes, placeholder: Some("[\"*\"]".to_string()), rows: Some(2), error: None }
            FormTextarea { label: t!("api-keys-ip-allowlist").to_string(), value: new_ip_allowlist, placeholder: Some("[\"10.0.0.0/8\"]".to_string()), rows: Some(2), error: None }
            FormInput { label: t!("api-keys-rate-limit").to_string(), value: new_rate_limit, placeholder: Some("requests per minute, e.g. 100".to_string()), input_type: Some("number".to_string()), error: None }
            if let Some(key) = created_key.read().as_ref() {
                Alert { level: "warning".to_string(),
                    p { "Copy this key now — it will not be shown again:" }
                    pre { class: "mono secret-box", "{key}" }
                }
            }
        }

        ConfirmDialog {
            title: t!("api-keys-delete-title").to_string(),
            message: t!("api-keys-delete-confirm").to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_api_key(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            keys.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

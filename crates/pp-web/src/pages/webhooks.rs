use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{Alert, ConfirmDialog, FormInput, FormTextarea, Modal, Pagination};

#[component]
pub fn Webhooks() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut hooks = use_resource(move || async move {
        api::get_webhooks_paginated(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });

    let mut show_modal = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut new_name = use_signal(String::new);
    let mut new_url = use_signal(String::new);
    let mut new_events = use_signal(|| "[\"client.created\", \"client.exceeded\"]".to_string());
    let mut new_secret = use_signal(String::new);
    let mut new_is_active = use_signal(|| true);

    let hooks_data = hooks
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = hooks
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut reset_form = move || {
        new_name.set(String::new());
        new_url.set(String::new());
        new_events.set("[\"client.created\", \"client.exceeded\"]".to_string());
        new_secret.set(String::new());
        new_is_active.set(true);
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("webhooks-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        error.set(None);
                        show_modal.set(true);
                    },
                    {t!("webhooks-create")}
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { {t!("common-name")} }
                        th { {t!("webhooks-url")} }
                        th { {t!("common-active")} }
                        th { {t!("common-actions")} }
                    }
                }
                tbody {
                    for h in hooks_data.iter() {
                        {
                            let id = h.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let name = h.get("name").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let url = h.get("url").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let is_active = h.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
                            let did = id.clone();
                            rsx! {
                                tr {
                                    td { "{name}" }
                                    td { class: "mono", "{url}" }
                                    td { "{is_active}" }
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
            title: t!("webhooks-create-title").to_string(),
            show: show_modal,
            on_confirm: move |_| {
                let name = new_name.read().clone();
                let url = new_url.read().clone();
                if name.is_empty() || url.is_empty() {
                    error.set(Some("Name and URL are required".to_string()));
                    return;
                }
                let events: Value = serde_json::from_str(&new_events.read())
                    .unwrap_or_else(|_| json!([]));
                let secret = new_secret.read().clone();
                let is_active = *new_is_active.read();
                spawn(async move {
                    let secret_opt = if secret.is_empty() { None } else { Some(secret) };
                    match api::create_webhook(
                        &name,
                        &url,
                        events,
                        secret_opt,
                        is_active,
                    ).await {
                        Ok(_) => {
                            reset_form();
                            show_modal.set(false);
                            hooks.restart();
                        }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            confirm_text: Some(t!("common-create").to_string()),
            FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("notify-slack".to_string()), input_type: None, error: None }
            FormInput { label: t!("webhooks-url").to_string(), value: new_url, placeholder: Some("https://hooks.example.com/webhook".to_string()), input_type: Some("url".to_string()), error: None }
            FormTextarea { label: t!("webhooks-events").to_string(), value: new_events, placeholder: Some("[\"client.created\"]".to_string()), rows: Some(2), error: None }
            FormInput { label: t!("webhooks-secret").to_string(), value: new_secret, placeholder: Some("HMAC secret".to_string()), input_type: Some("password".to_string()), error: None }
            div { class: "form-group",
                label {
                    input {
                        r#type: "checkbox",
                        checked: "{new_is_active.read().clone()}",
                        onchange: move |e| new_is_active.set(e.checked()),
                    }
                    " "
                    {t!("common-active")}
                }
            }
        }

        ConfirmDialog {
            title: t!("webhooks-delete-title").to_string(),
            message: t!("webhooks-delete-confirm").to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_webhook(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            hooks.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

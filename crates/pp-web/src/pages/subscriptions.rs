use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{Alert, ConfirmDialog, FormDate, FormInput, FormSelect, FormTextarea, Modal, Pagination};

fn parse_json_object(s: &str) -> Result<Option<Value>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed).map(Some).map_err(|e| format!("Invalid JSON: {}", e))
}

fn mask_token(s: &str) -> String {
    if s.len() <= 8 {
        s.to_string()
    } else {
        format!("{}••••{}", &s[..4], &s[s.len()-4..])
    }
}

#[component]
pub fn Subscriptions() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut subs = use_resource(move || async move {
        api::get_subscriptions_paginated(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });
    let clients = use_resource(|| async move { api::get_clients().await.unwrap_or_default() });
    let mut templates =
        use_resource(|| async move { api::get_templates().await.unwrap_or_default() });

    let mut show_create = use_signal(|| false);
    let mut show_template_modal = use_signal(|| false);
    let mut show_edit = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut edit_id = use_signal(String::new);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut selected_client = use_signal(String::new);
    let mut selected_template = use_signal(String::new);

    // Auto-select the first available client/template so single-option forms work
    // without requiring the user to manually trigger onchange.
    use_effect(move || {
        let _ = clients.read();
        if selected_client.read().is_empty() {
            if let Some(id) = clients
                .read()
                .as_ref()
                .and_then(|r| r.get("data"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("id"))
                .and_then(|v| v.as_str())
            {
                selected_client.set(id.to_string());
            }
        }
    });

    use_effect(move || {
        let _ = templates.read();
        if selected_template.read().is_empty() {
            if let Some(id) = templates
                .read()
                .as_ref()
                .and_then(|r| r.get("data"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|t| t.get("id"))
                .and_then(|v| v.as_str())
            {
                selected_template.set(id.to_string());
            }
        }
    });

    // Template form
    let mut new_template_name = use_signal(String::new);
    let mut new_template_format = use_signal(|| "base64".to_string());
    let mut new_template_base_config = use_signal(|| "{}".to_string());
    let mut new_template_filter_rules = use_signal(|| "[]".to_string());
    let mut new_template_custom_headers = use_signal(|| "{}".to_string());

    // Subscription edit form
    let mut edit_is_active = use_signal(|| true);
    let mut edit_expire_at = use_signal(String::new);

    let client_map: std::collections::HashMap<String, String> = clients
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| {
            let id = c.get("id")?.as_str()?.to_string();
            let name = c.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let template_map: std::collections::HashMap<String, String> = templates
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| {
            let id = t.get("id")?.as_str()?.to_string();
            let name = t.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let client_options: Vec<(String, String)> = clients
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| {
            let id = c.get("id")?.as_str()?.to_string();
            let name = c.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let template_options: Vec<(String, String)> = templates
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| {
            let id = t.get("id")?.as_str()?.to_string();
            let name = t.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let subs_data = subs
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = subs
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut reset_template_form = move || {
        new_template_name.set(String::new());
        new_template_format.set("base64".to_string());
        new_template_base_config.set("{}".to_string());
        new_template_filter_rules.set("[]".to_string());
        new_template_custom_headers.set("{}".to_string());
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("subscriptions-title")} }
                div { class: "actions",
                    button {
                        onclick: move |_| {
                            reset_template_form();
                            error.set(None);
                            show_template_modal.set(true);
                        },
                        {t!("subscriptions-template-create")}
                    }
                    button {
                        onclick: move |_| {
                            selected_client.set(String::new());
                            selected_template.set(String::new());
                            error.set(None);
                            show_create.set(true);
                        },
                        {t!("subscriptions-create")}
                    }
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if subs_data.is_empty() {
                div { class: "empty-state", p { "No subscriptions found." } }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("nav-clients")} }
                            th { {t!("subscriptions-template")} }
                            th { {t!("subscriptions-token")} }
                            th { {t!("subscriptions-url-path")} }
                            th { {t!("subscriptions-is-active")} }
                            th { {t!("subscriptions-expire-at")} }
                            th { {t!("subscriptions-last-accessed")} }
                            th { {t!("common-actions")} }
                        }
                    }
                    tbody {
                        for s in subs_data.iter() {
                            {
                                let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let client_id = s.get("client_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let template_id = s.get("template_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let client_name = client_map.get(&client_id).cloned().unwrap_or_else(|| "-".to_string());
                                let template_name = template_map.get(&template_id).cloned().unwrap_or_else(|| "-".to_string());
                                let token = s.get("token").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let url_path = s.get("url_path").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let is_active = s.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
                                let expire_at = s.get("expire_at").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let last_accessed = s.get("last_accessed_at").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let did = id.clone();
                                let edit_id_clone = id.clone();
                                let edit_is_active_initial = is_active;
                                let edit_expire_initial = expire_at.clone();
                                rsx! {
                                    tr {
                                        td { "{client_name}" }
                                        td { "{template_name}" }
                                        td { class: "mono", "{mask_token(&token)}" }
                                        td { class: "mono", "{url_path}" }
                                        td { "{is_active}" }
                                        td { "{expire_at}" }
                                        td { "{last_accessed}" }
                                        td {
                                            button {
                                                onclick: move |_| {
                                                    edit_id.set(edit_id_clone.clone());
                                                    edit_is_active.set(edit_is_active_initial);
                                                    edit_expire_at.set(edit_expire_initial.clone());
                                                    error.set(None);
                                                    show_edit.set(true);
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
            title: t!("subscriptions-create-title").to_string(),
            show: show_create,
            on_confirm: move |_| {
                let client_id = selected_client.read().clone();
                let template_id = selected_template.read().clone();
                if client_id.is_empty() || template_id.is_empty() {
                    error.set(Some(t!("common-required-field").to_string()));
                    return;
                }
                spawn(async move {
                    match api::create_subscription(&client_id, &template_id).await {
                        Ok(_) => {
                            selected_client.set(String::new());
                            selected_template.set(String::new());
                            show_create.set(false);
                            subs.restart();
                        }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            confirm_text: Some(t!("common-create").to_string()),
            FormSelect {
                label: t!("nav-clients").to_string(),
                value: selected_client,
                options: client_options,
                error: None,
            }
            FormSelect {
                label: t!("subscriptions-template").to_string(),
                value: selected_template,
                options: template_options,
                error: None,
            }
        }

        Modal {
            title: t!("subscriptions-template-create-title").to_string(),
            show: show_template_modal,
            on_confirm: move |_| {
                let name = new_template_name.read().clone();
                if name.is_empty() {
                    error.set(Some(t!("common-required-field").to_string()));
                    return;
                }
                let base_config = match parse_json_object(&new_template_base_config.read()) {
                    Ok(v) => v,
                    Err(e) => { error.set(Some(e)); return; }
                };
                let filter_rules = match parse_json_object(&new_template_filter_rules.read()) {
                    Ok(v) => v,
                    Err(e) => { error.set(Some(e)); return; }
                };
                let custom_headers = match parse_json_object(&new_template_custom_headers.read()) {
                    Ok(v) => v,
                    Err(e) => { error.set(Some(e)); return; }
                };
                let format = new_template_format.read().clone();
                spawn(async move {
                    match api::create_template(
                        &name,
                        &format,
                        base_config,
                        filter_rules,
                        custom_headers,
                    ).await {
                        Ok(_) => {
                            reset_template_form();
                            show_template_modal.set(false);
                            templates.restart();
                        }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            confirm_text: Some(t!("common-create").to_string()),
            FormInput { label: t!("common-name").to_string(), value: new_template_name, placeholder: Some("base64-default".to_string()), input_type: None, error: None }
            FormSelect {
                label: t!("subscriptions-format").to_string(),
                value: new_template_format,
                options: vec![
                    ("base64".to_string(), "Base64".to_string()),
                    ("json".to_string(), "JSON".to_string()),
                    ("clash".to_string(), "Clash".to_string()),
                    ("sing-box".to_string(), "sing-box".to_string()),
                    ("v2rayng".to_string(), "V2RayNG".to_string()),
                ],
                error: None,
            }
            FormTextarea { label: t!("subscriptions-base-config").to_string(), value: new_template_base_config, placeholder: Some("{\"log\": {}}".to_string()), rows: Some(3), error: None }
            FormTextarea { label: t!("subscriptions-filter-rules").to_string(), value: new_template_filter_rules, placeholder: Some("[{\"field\": \"protocol\", \"op\": \"eq\", \"value\": \"vless_reality\"}]".to_string()), rows: Some(3), error: None }
            FormTextarea { label: t!("subscriptions-custom-headers").to_string(), value: new_template_custom_headers, placeholder: Some("{\"Content-Type\": \"text/plain\"}".to_string()), rows: Some(2), error: None }
        }

        Modal {
            title: t!("subscriptions-edit-title").to_string(),
            show: show_edit,
            on_confirm: move |_| {
                let id = edit_id.read().clone();
                let is_active = *edit_is_active.read();
                let expire_at = edit_expire_at.read().clone();
                let mut payload = json!({ "is_active": is_active });
                if !expire_at.is_empty() {
                    payload["expire_at"] = json!(expire_at);
                }
                spawn(async move {
                    match api::update_subscription(&id, payload).await {
                        Ok(_) => {
                            show_edit.set(false);
                            subs.restart();
                        }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            confirm_text: Some(t!("common-update").to_string()),
            div { class: "form-group",
                label {
                    input {
                        r#type: "checkbox",
                        checked: "{edit_is_active.read().clone()}",
                        onchange: move |e| edit_is_active.set(e.checked()),
                    }
                    " "
                    {t!("subscriptions-is-active")}
                }
            }
            FormDate { label: t!("subscriptions-expire-at").to_string(), value: edit_expire_at, error: None }
        }

        ConfirmDialog {
            title: t!("subscriptions-delete-title").to_string(),
            message: t!("subscriptions-delete-confirm").to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_subscription(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            subs.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

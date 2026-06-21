use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{Alert, ConfirmDialog, FormInput, FormTextarea, Modal, Pagination};

fn parse_json_object(s: &str) -> Result<Value, String> {
    if s.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(s).map_err(|e| format!("Invalid JSON: {}", e))
}

#[component]
pub fn Groups() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut groups = use_resource(move || async move {
        api::get_groups_paginated(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });
    let mut show_modal = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut is_edit = use_signal(|| false);
    let mut edit_id = use_signal(String::new);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut new_name = use_signal(String::new);
    let mut new_description = use_signal(String::new);
    let mut new_labels = use_signal(|| "{}".to_string());

    let groups_data = groups
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = groups
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut reset_form = move || {
        new_name.set(String::new());
        new_description.set(String::new());
        new_labels.set("{}".to_string());
    };

    let mut load_form = move |g: &Value| {
        new_name.set(g.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string());
        new_description.set(g.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string());
        if let Some(labels) = g.get("labels").and_then(|v| v.as_object()) {
            new_labels.set(serde_json::to_string_pretty(labels).unwrap_or_else(|_| "{}".to_string()));
        } else {
            new_labels.set("{}".to_string());
        }
    };

    let modal_title = if *is_edit.read() {
        t!("groups-edit-title").to_string()
    } else {
        t!("groups-create-title").to_string()
    };
    let confirm_text = if *is_edit.read() {
        t!("common-update").to_string()
    } else {
        t!("common-create").to_string()
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("groups-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        error.set(None);
                        is_edit.set(false);
                        show_modal.set(true);
                    },
                    {t!("groups-create")}
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if groups_data.is_empty() {
                div { class: "empty-state", p { "No groups found." } }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("common-name")} }
                            th { {t!("groups-description")} }
                            th { {t!("common-labels")} }
                            th { {t!("common-actions")} }
                        }
                    }
                    tbody {
                        for g in groups_data.iter() {
                            {
                                let id = g.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let name = g.get("name").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let description = g.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let labels = g.get("labels").cloned().unwrap_or(json!({}));
                                let did = id.clone();
                                let edit_id_clone = id.clone();
                                let edit_data = g.clone();
                                rsx! {
                                    tr {
                                        td { "{name}" }
                                        td { "{description}" }
                                        td { class: "mono", "{labels}" }
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
                let name = new_name.read().clone();
                if name.is_empty() {
                    error.set(Some(t!("common-required-field").to_string()));
                    return;
                }
                let labels = match parse_json_object(&new_labels.read()) {
                    Ok(v) => v,
                    Err(e) => { error.set(Some(e)); return; }
                };
                let description = new_description.read().clone();
                let desc_opt = if description.is_empty() { None } else { Some(description) };
                if *is_edit.read() {
                    let id = edit_id.read().clone();
                    spawn(async move {
                        let mut payload = json!({ "name": name, "labels": labels });
                        if let Some(d) = desc_opt {
                            payload["description"] = json!(d);
                        }
                        match api::update_group(&id, payload).await {
                            Ok(_) => {
                                reset_form();
                                is_edit.set(false);
                                show_modal.set(false);
                                groups.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                } else {
                    spawn(async move {
                        match api::create_group(&name, desc_opt, Some(labels)).await {
                            Ok(_) => {
                                reset_form();
                                show_modal.set(false);
                                groups.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                }
            },
            confirm_text: Some(confirm_text),
            FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("Europe".to_string()), input_type: None, error: None }
            FormInput { label: t!("groups-description").to_string(), value: new_description, placeholder: Some("European nodes".to_string()), input_type: None, error: None }
            FormTextarea { label: t!("groups-labels").to_string(), value: new_labels, placeholder: Some("{\"region\": \"eu\"}".to_string()), rows: Some(3), error: None }
        }

        ConfirmDialog {
            title: t!("common-confirm-delete").to_string(),
            message: "Delete this group?".to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_group(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            groups.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

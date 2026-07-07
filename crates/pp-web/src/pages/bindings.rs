use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{
    Alert, ConfirmDialog, FormSelect, FormTextarea, Modal, Pagination, StatusBadge,
};

fn parse_json_object(s: &str) -> Result<Option<Value>, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|e| format!("Invalid JSON: {}", e))
}

#[component]
pub fn Bindings() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut bindings = use_resource(move || async move {
        api::get_bindings_paginated(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });
    let nodes = use_resource(|| async move { api::get_nodes().await.unwrap_or_default() });
    let protocols =
        use_resource(|| async move { api::get_all_protocols().await.unwrap_or_default() });

    let mut show_modal = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut selected_node = use_signal(String::new);
    let mut selected_protocol = use_signal(String::new);
    let mut is_active = use_signal(|| true);
    let mut override_settings = use_signal(|| "{}".to_string());

    // Auto-select the first available node/protocol so single-option forms work
    // without requiring the user to manually trigger onchange.
    use_effect(move || {
        let _ = nodes.read();
        if selected_node.read().is_empty() {
            if let Some(id) = nodes
                .read()
                .as_ref()
                .and_then(|r| r.get("data"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|n| n.get("id"))
                .and_then(|v| v.as_str())
            {
                selected_node.set(id.to_string());
            }
        }
    });

    use_effect(move || {
        let _ = protocols.read();
        if selected_protocol.read().is_empty() {
            if let Some(id) = protocols
                .read()
                .as_ref()
                .and_then(|r| r.get("data"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|p| p.get("id"))
                .and_then(|v| v.as_str())
            {
                selected_protocol.set(id.to_string());
            }
        }
    });

    let node_map: std::collections::HashMap<String, String> = nodes
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|n| {
            let id = n.get("id")?.as_str()?.to_string();
            let name = n.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let protocol_map: std::collections::HashMap<String, String> = protocols
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| {
            let id = p.get("id")?.as_str()?.to_string();
            let name = p.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let node_options: Vec<(String, String)> = nodes
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|n| {
            let id = n.get("id")?.as_str()?.to_string();
            let name = n.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let protocol_options: Vec<(String, String)> = protocols
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| {
            let id = p.get("id")?.as_str()?.to_string();
            let name = p.get("name")?.as_str()?.to_string();
            Some((id, name))
        })
        .collect();

    let bindings_data = bindings
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = bindings
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut reset_form = move || {
        selected_node.set(String::new());
        selected_protocol.set(String::new());
        is_active.set(true);
        override_settings.set("{}".to_string());
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("bindings-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        error.set(None);
                        show_modal.set(true);
                    },
                    {t!("bindings-create")}
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if bindings_data.is_empty() {
                div { class: "empty-state", p { "No bindings found." } }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("bindings-node")} }
                            th { {t!("bindings-protocol")} }
                            th { {t!("common-active")} }
                            th { {t!("bindings-override-settings")} }
                            th { {t!("common-actions")} }
                        }
                    }
                    tbody {
                        for b in bindings_data.iter() {
                            {
                                let id = b.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let node_id = b.get("node_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let protocol_id = b.get("protocol_config_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let node_name = node_map.get(&node_id).cloned().unwrap_or_else(|| "-".to_string());
                                let protocol_name = protocol_map.get(&protocol_id).cloned().unwrap_or_else(|| "-".to_string());
                                let active = b.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
                                let status = if active { "active" } else { "inactive" };
                                let overrides = b.get("override_settings").cloned().unwrap_or(json!({}));
                                let did = id.clone();
                                rsx! {
                                    tr {
                                        td { "{node_name}" }
                                        td { "{protocol_name}" }
                                        td {
                                            StatusBadge { status: status.to_string() }
                                        }
                                        td { class: "mono", "{overrides}" }
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
            title: t!("bindings-create-title").to_string(),
            show: show_modal,
            on_confirm: move |_| {
                let node_id = selected_node.read().clone();
                let protocol_id = selected_protocol.read().clone();
                let active = *is_active.read();
                let overrides = match parse_json_object(&override_settings.read()) {
                    Ok(v) => v,
                    Err(e) => { error.set(Some(e)); return; }
                };
                if node_id.is_empty() || protocol_id.is_empty() {
                    error.set(Some(t!("common-required-field").to_string()));
                    return;
                }
                spawn(async move {
                    match api::create_binding(&node_id, &protocol_id, active, overrides
                    ).await {
                        Ok(_) => {
                            reset_form();
                            show_modal.set(false);
                            bindings.restart();
                        }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            confirm_text: Some(t!("common-create").to_string()),
            FormSelect {
                label: t!("bindings-node").to_string(),
                value: selected_node,
                options: node_options,
                error: None,
            }
            FormSelect {
                label: t!("bindings-protocol").to_string(),
                value: selected_protocol,
                options: protocol_options,
                error: None,
            }
            div { class: "form-group",
                label {
                    input {
                        r#type: "checkbox",
                        checked: "{is_active.read().clone()}",
                        onchange: move |e| is_active.set(e.checked()),
                    }
                    " "
                    {t!("common-active")}
                }
            }
            FormTextarea { label: t!("bindings-override-settings").to_string(), value: override_settings, placeholder: Some("{\"listen_port\": 8443}".to_string()), rows: Some(3), error: None }
        }

        ConfirmDialog {
            title: t!("bindings-delete-title").to_string(),
            message: t!("bindings-delete-confirm").to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_binding(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            bindings.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

use dioxus::prelude::*;
use serde_json::Value;

use crate::api;
use crate::components::{FormSelect, Modal, StatusBadge};

#[component]
pub fn Bindings() -> Element {
    let mut bindings = use_resource(|| async move { api::get_bindings().await.unwrap_or_default() });
    let mut nodes = use_resource(|| async move { api::get_nodes().await.unwrap_or_default() });
    let mut protocols = use_resource(|| async move { api::get_protocols(1, 100).await.unwrap_or_default() });
    let mut show_create = use_signal(|| false);
    let mut selected_node = use_signal(|| String::new());
    let mut selected_protocol = use_signal(|| String::new());
    let mut is_active = use_signal(|| true);

    let node_map: std::collections::HashMap<String, String> = nodes.read()
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

    let protocol_map: std::collections::HashMap<String, String> = protocols.read()
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

    let node_options: Vec<(String, String)> = nodes.read()
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

    let protocol_options: Vec<(String, String)> = protocols.read()
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

    let bindings_data = bindings.read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { "Node Bindings" }
                button {
                    onclick: move |_| show_create.set(true),
                    "+ Create Binding"
                }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { "Node" }
                        th { "Protocol" }
                        th { "Active" }
                        th { "Actions" }
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
                            let delete_id = id.clone();
                            rsx! {
                                tr {
                                    td { "{node_name}" }
                                    td { "{protocol_name}" }
                                    td {
                                        StatusBadge { status: status.to_string() }
                                    }
                                    td {
                                        button {
                                            class: "danger",
                                            onclick: move |_| {
                                                let id = delete_id.clone();
                                                spawn(async move {
                                                    let _ = api::delete_binding(&id).await;
                                                    bindings.restart();
                                                });
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Modal {
            title: "Create Binding".to_string(),
            show: show_create,
            on_confirm: move |_| {
                let node_id = selected_node.read().clone();
                let protocol_id = selected_protocol.read().clone();
                let active = is_active.read().clone();
                if !node_id.is_empty() && !protocol_id.is_empty() {
                    spawn(async move {
                        let _ = api::create_binding(&node_id, &protocol_id, active).await;
                        selected_node.set(String::new());
                        selected_protocol.set(String::new());
                        is_active.set(true);
                        show_create.set(false);
                        bindings.restart();
                    });
                }
            },
            confirm_text: Some("Create".to_string()),
            FormSelect {
                label: "Node".to_string(),
                value: selected_node,
                options: node_options,
            }
            FormSelect {
                label: "Protocol".to_string(),
                value: selected_protocol,
                options: protocol_options,
            }
            div { class: "form-group",
                label {
                    input {
                        r#type: "checkbox",
                        checked: "{is_active.read().clone()}",
                        onchange: move |e| is_active.set(e.checked()),
                    }
                    " Active"
                }
            }
        }
    }
}

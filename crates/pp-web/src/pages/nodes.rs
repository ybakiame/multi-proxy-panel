use dioxus::prelude::*;
use serde_json::Value;

use crate::api;
use crate::components::{FormInput, Modal, StatusBadge};

#[component]
pub fn Nodes() -> Element {
    let mut nodes = use_resource(|| async move { api::get_nodes().await.unwrap_or_default() });
    let mut show_create = use_signal(|| false);
    let mut new_name = use_signal(|| String::new());
    let mut new_hostname = use_signal(|| String::new());
    let mut new_address = use_signal(|| String::new());

    let nodes_data = nodes.read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { "Nodes" }
                button {
                    onclick: move |_| show_create.set(true),
                    "+ Create Node"
                }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { "Name" }
                        th { "Hostname" }
                        th { "Address" }
                        th { "Status" }
                        th { "Actions" }
                    }
                }
                tbody {
                    for node in nodes_data.iter() {
                        {
                            let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let hostname = node.get("hostname").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let address = node.get("address").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let status = node.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                            let push_id = id.clone();
                            let delete_id = id.clone();
                            rsx! {
                                tr {
                                    td { "{name}" }
                                    td { "{hostname}" }
                                    td { "{address}" }
                                    td {
                                        StatusBadge { status: status.clone() }
                                    }
                                    td {
                                        button {
                                            onclick: move |_| {
                                                let id = push_id.clone();
                                                spawn(async move {
                                                    let payload = serde_json::json!({ "core_type": "sing-box", "restart": true, "version": "1" });
                                                    let _ = api::push_config(&id, payload).await;
                                                });
                                            },
                                            "Push Config"
                                        }
                                        button {
                                            class: "danger",
                                            onclick: move |_| {
                                                let id = delete_id.clone();
                                                spawn(async move {
                                                    let _ = api::delete_node(&id).await;
                                                    nodes.restart();
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
            title: "Create Node".to_string(),
            show: show_create,
            on_confirm: move |_| {
                let name = new_name.read().clone();
                let hostname = new_hostname.read().clone();
                let address = new_address.read().clone();
                if !name.is_empty() {
                    spawn(async move {
                        let _ = api::create_node(&name, &hostname, &address).await;
                        new_name.set(String::new());
                        new_hostname.set(String::new());
                        new_address.set(String::new());
                        show_create.set(false);
                        nodes.restart();
                    });
                }
            },
            confirm_text: Some("Create".to_string()),
            FormInput { label: "Name".to_string(), value: new_name, placeholder: Some("node-01".to_string()), input_type: None }
            FormInput { label: "Hostname".to_string(), value: new_hostname, placeholder: Some("host.example.com".to_string()), input_type: None }
            FormInput { label: "Address".to_string(), value: new_address, placeholder: Some("192.168.1.1".to_string()), input_type: None }
        }
    }
}

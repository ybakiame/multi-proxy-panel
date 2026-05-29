use dioxus::prelude::*;
use serde_json::Value;

use crate::api;
use crate::components::{FormSelect, Modal};

#[component]
pub fn Subscriptions() -> Element {
    let mut subs = use_resource(|| async move { api::get_subscriptions().await.unwrap_or_default() });
    let mut clients = use_resource(|| async move { api::get_clients().await.unwrap_or_default() });
    let mut templates = use_resource(|| async move { api::get_templates().await.unwrap_or_default() });
    let mut show_create = use_signal(|| false);
    let mut selected_client = use_signal(|| String::new());
    let mut selected_template = use_signal(|| String::new());

    let client_map: std::collections::HashMap<String, String> = clients.read()
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

    let template_map: std::collections::HashMap<String, String> = templates.read()
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

    let client_options: Vec<(String, String)> = clients.read()
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

    let template_options: Vec<(String, String)> = templates.read()
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

    let subs_data = subs.read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { "Subscriptions" }
                button {
                    onclick: move |_| show_create.set(true),
                    "+ Create Subscription"
                }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { "Client" }
                        th { "Template" }
                        th { "Token" }
                        th { "URL Path" }
                        th { "Active" }
                        th { "Actions" }
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
                            let delete_id = id.clone();
                            rsx! {
                                tr {
                                    td { "{client_name}" }
                                    td { "{template_name}" }
                                    td { class: "mono", "{token}" }
                                    td { class: "mono", "{url_path}" }
                                    td { "{is_active}" }
                                    td {
                                        button {
                                            class: "danger",
                                            onclick: move |_| {
                                                let id = delete_id.clone();
                                                spawn(async move {
                                                    let _ = api::delete_subscription(&id).await;
                                                    subs.restart();
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
            title: "Create Subscription".to_string(),
            show: show_create,
            on_confirm: move |_| {
                let client_id = selected_client.read().clone();
                let template_id = selected_template.read().clone();
                if !client_id.is_empty() && !template_id.is_empty() {
                    spawn(async move {
                        let _ = api::create_subscription(&client_id, &template_id).await;
                        selected_client.set(String::new());
                        selected_template.set(String::new());
                        show_create.set(false);
                        subs.restart();
                    });
                }
            },
            confirm_text: Some("Create".to_string()),
            FormSelect {
                label: "Client".to_string(),
                value: selected_client,
                options: client_options,
            }
            FormSelect {
                label: "Template".to_string(),
                value: selected_template,
                options: template_options,
            }
        }
    }
}

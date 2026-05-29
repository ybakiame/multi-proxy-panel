use dioxus::prelude::*;
use serde_json::Value;

use crate::api;
use crate::components::{FormInput, Modal, StatusBadge};

#[component]
pub fn Clients() -> Element {
    let mut clients = use_resource(|| async move { api::get_clients().await.unwrap_or_default() });
    let mut show_create = use_signal(|| false);
    let mut new_name = use_signal(|| String::new());
    let mut new_email = use_signal(|| String::new());
    let mut new_limit = use_signal(|| "0".to_string());
    let mut new_reset_day = use_signal(|| String::new());

    let clients_data = clients.read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { "Clients" }
                button {
                    onclick: move |_| show_create.set(true),
                    "+ Create Client"
                }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { "Name" }
                        th { "Email" }
                        th { "Status" }
                        th { "Traffic Used" }
                        th { "Traffic Limit" }
                        th { "Actions" }
                    }
                }
                tbody {
                    for c in clients_data.iter() {
                        {
                            let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let email = c.get("email").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                            let used = c.get("traffic_used_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                            let limit = c.get("traffic_limit_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                            let delete_id = id.clone();
                            rsx! {
                                tr {
                                    td { "{name}" }
                                    td { "{email}" }
                                    td {
                                        StatusBadge { status: status.clone() }
                                    }
                                    td { "{used}" }
                                    td { "{limit}" }
                                    td {
                                        button {
                                            class: "danger",
                                            onclick: move |_| {
                                                let id = delete_id.clone();
                                                spawn(async move {
                                                    let _ = api::delete_client(&id).await;
                                                    clients.restart();
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
            title: "Create Client".to_string(),
            show: show_create,
            on_confirm: move |_| {
                let name = new_name.read().clone();
                let email = new_email.read().clone();
                let limit = new_limit.read().parse::<i64>().unwrap_or(0);
                let reset_day = new_reset_day.read().parse::<i32>().ok();
                if !name.is_empty() {
                    spawn(async move {
                        let email_opt = if email.is_empty() { None } else { Some(email.as_str()) };
                        let _ = api::create_client(&name, email_opt, limit, reset_day).await;
                        new_name.set(String::new());
                        new_email.set(String::new());
                        new_limit.set("0".to_string());
                        new_reset_day.set(String::new());
                        show_create.set(false);
                        clients.restart();
                    });
                }
            },
            confirm_text: Some("Create".to_string()),
            FormInput { label: "Name".to_string(), value: new_name, placeholder: Some("user01".to_string()), input_type: None }
            FormInput { label: "Email".to_string(), value: new_email, placeholder: Some("user@example.com".to_string()), input_type: Some("email".to_string()) }
            FormInput { label: "Traffic Limit (bytes)".to_string(), value: new_limit, placeholder: Some("1073741824".to_string()), input_type: Some("number".to_string()) }
            FormInput { label: "Reset Day (1-31)".to_string(), value: new_reset_day, placeholder: Some("1".to_string()), input_type: Some("number".to_string()) }
        }
    }
}

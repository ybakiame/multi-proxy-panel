use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api;
use crate::components::{FormInput, Modal, StatusBadge};

#[component]
pub fn Clients() -> Element {
    let mut clients = use_resource(|| async move { api::get_clients().await.unwrap_or_default() });
    let groups = use_resource(|| async move { api::get_groups().await.unwrap_or_default() });
    let mut show_create = use_signal(|| false);
    let mut new_name = use_signal(String::new);
    let mut new_email = use_signal(String::new);
    let mut new_limit = use_signal(|| "0".to_string());
    let mut new_reset_day = use_signal(String::new);
    let mut new_reset_strategy = use_signal(|| "no_reset".to_string());
    let mut selected_group_ids = use_signal(Vec::<String>::new);

    let clients_data = clients
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let groups_data = groups
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("clients-title")} }
                button {
                    onclick: move |_| {
                        selected_group_ids.set(Vec::new());
                        new_reset_strategy.set("no_reset".to_string());
                        show_create.set(true);
                    },
                    {t!("clients-create")}
                }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { {t!("common-name")} }
                        th { {t!("clients-email")} }
                        th { {t!("common-status")} }
                        th { {t!("clients-traffic-used")} }
                        th { {t!("clients-traffic-limit")} }
                        th { {t!("clients-reset-strategy")} }
                        th { {t!("clients-groups")} }
                        th { {t!("common-actions")} }
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
                            let strategy = c.get("data_limit_reset_strategy").and_then(|v| v.as_str()).unwrap_or("no_reset").to_string();
                            let group_ids = c.get("group_ids").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                            let group_names: Vec<String> = group_ids.iter()
                                .filter_map(|gid| gid.as_str())
                                .filter_map(|gid| {
                                    groups_data.iter().find(|g| {
                                        g.get("id").and_then(|v| v.as_str()) == Some(gid)
                                    }).and_then(|g| g.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                                })
                                .collect();
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
                                    td { "{strategy}" }
                                    td {
                                        if group_names.is_empty() {
                                            "-"
                                        } else {
                                            "{group_names.join(\", \")}"
                                        }
                                    }
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

        Modal {
            title: t!("clients-create-title").to_string(),
            show: show_create,
            on_confirm: move |_| {
                let name = new_name.read().clone();
                let email = new_email.read().clone();
                let limit = new_limit.read().parse::<i64>().unwrap_or(0);
                let reset_day = new_reset_day.read().parse::<i32>().ok();
                let strategy = new_reset_strategy.read().clone();
                let group_ids = selected_group_ids.read().clone();
                if !name.is_empty() {
                    spawn(async move {
                        let email_opt = if email.is_empty() { None } else { Some(email.as_str()) };
                        let _ = api::create_client(&name, email_opt, limit, reset_day, &strategy, group_ids).await;
                        new_name.set(String::new());
                        new_email.set(String::new());
                        new_limit.set("0".to_string());
                        new_reset_day.set(String::new());
                        new_reset_strategy.set("no_reset".to_string());
                        selected_group_ids.set(Vec::new());
                        show_create.set(false);
                        clients.restart();
                    });
                }
            },
            confirm_text: Some(t!("common-create").to_string()),
            FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("user01".to_string()), input_type: None }
            FormInput { label: t!("clients-email").to_string(), value: new_email, placeholder: Some("user@example.com".to_string()), input_type: Some("email".to_string()) }
            FormInput { label: format!("{} (bytes)", t!("clients-traffic-limit")), value: new_limit, placeholder: Some("1073741824".to_string()), input_type: Some("number".to_string()) }
            FormInput { label: format!("{} (1-31)", t!("clients-reset-day")), value: new_reset_day, placeholder: Some("1".to_string()), input_type: Some("number".to_string()) }
            div { class: "form-group",
                label { {t!("clients-reset-strategy")} }
                select {
                    class: "form-select",
                    onchange: move |e| {
                        new_reset_strategy.set(e.value());
                    },
                    option { value: "no_reset", selected: new_reset_strategy.read().clone() == "no_reset", "no_reset" }
                    option { value: "daily", selected: new_reset_strategy.read().clone() == "daily", "daily" }
                    option { value: "weekly", selected: new_reset_strategy.read().clone() == "weekly", "weekly" }
                    option { value: "monthly", selected: new_reset_strategy.read().clone() == "monthly", "monthly" }
                    option { value: "yearly", selected: new_reset_strategy.read().clone() == "yearly", "yearly" }
                }
            }
            div { class: "form-group",
                label { {t!("clients-groups")} }
                div { class: "checkbox-group",
                    for g in groups_data.iter() {
                        {
                            let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let gname = g.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let checkbox_gid = gid.clone();
                            let mut selected = selected_group_ids;
                            rsx! {
                                label { class: "checkbox-label",
                                    input {
                                        r#type: "checkbox",
                                        value: "{checkbox_gid}",
                                        onchange: move |e| {
                                            let mut current = selected.read().clone();
                                            if e.checked() {
                                                if !current.contains(&checkbox_gid) {
                                                    current.push(checkbox_gid.clone());
                                                }
                                            } else {
                                                current.retain(|id| id != &checkbox_gid);
                                            }
                                            selected.set(current);
                                        }
                                    }
                                    "{gname}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

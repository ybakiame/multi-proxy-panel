use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{
    Alert, ConfirmDialog, FormDate, FormInput, FormSelect, Modal, Pagination, StatusBadge,
};

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[component]
pub fn Clients() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let mut clients = use_resource(move || async move {
        api::get_clients_paginated(*page.read(), *per_page.read())
            .await
            .unwrap_or_default()
    });
    let groups = use_resource(|| async move { api::get_groups().await.unwrap_or_default() });

    let mut show_modal = use_signal(|| false);
    let mut show_delete = use_signal(|| false);
    let mut is_edit = use_signal(|| false);
    let mut edit_id = use_signal(String::new);
    let mut delete_id = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut new_name = use_signal(String::new);
    let mut new_email = use_signal(String::new);
    let mut new_user_id = use_signal(String::new);
    let mut new_limit = use_signal(|| "0".to_string());
    let mut new_expiry = use_signal(String::new);
    let mut new_reset_day = use_signal(String::new);
    let mut new_reset_strategy = use_signal(|| "no_reset".to_string());
    let mut new_max_devices = use_signal(String::new);
    let mut new_status = use_signal(|| "active".to_string());
    let mut new_on_hold_duration = use_signal(String::new);
    let mut new_on_hold_timeout = use_signal(String::new);
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

    let total = clients
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut reset_form = move || {
        new_name.set(String::new());
        new_email.set(String::new());
        new_user_id.set(String::new());
        new_limit.set("0".to_string());
        new_expiry.set(String::new());
        new_reset_day.set(String::new());
        new_reset_strategy.set("no_reset".to_string());
        new_max_devices.set(String::new());
        new_status.set("active".to_string());
        new_on_hold_duration.set(String::new());
        new_on_hold_timeout.set(String::new());
        selected_group_ids.set(Vec::new());
    };

    let mut load_form = move |c: &Value| {
        new_name.set(
            c.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        new_email.set(
            c.get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        new_user_id.set(
            c.get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        new_limit.set(
            c.get("traffic_limit_bytes")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .to_string(),
        );
        new_expiry.set(
            c.get("expiry_date")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        new_reset_day.set(
            c.get("reset_day")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default(),
        );
        new_reset_strategy.set(
            c.get("data_limit_reset_strategy")
                .and_then(|v| v.as_str())
                .unwrap_or("no_reset")
                .to_string(),
        );
        new_max_devices.set(
            c.get("max_devices")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default(),
        );
        new_status.set(
            c.get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("active")
                .to_string(),
        );
        new_on_hold_duration.set(
            c.get("on_hold_expire_duration_secs")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default(),
        );
        new_on_hold_timeout.set(
            c.get("on_hold_timeout")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        );
        let gids: Vec<String> = c
            .get("group_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        selected_group_ids.set(gids);
    };

    let modal_title = if *is_edit.read() {
        t!("clients-edit-title").to_string()
    } else {
        t!("clients-create-title").to_string()
    };
    let confirm_text = if *is_edit.read() {
        t!("common-update").to_string()
    } else {
        t!("common-create").to_string()
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("clients-title")} }
                button {
                    onclick: move |_| {
                        reset_form();
                        error.set(None);
                        is_edit.set(false);
                        show_modal.set(true);
                    },
                    {t!("clients-create")}
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if clients_data.is_empty() {
                div { class: "empty-state", p { "No clients found." } }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("common-name")} }
                            th { {t!("clients-email")} }
                            th { {t!("clients-user-id")} }
                            th { {t!("common-status")} }
                            th { {t!("clients-traffic-used")} }
                            th { {t!("clients-traffic-limit")} }
                            th { {t!("clients-all-time-used")} }
                            th { {t!("clients-is-exceeded")} }
                            th { {t!("clients-on-hold")} }
                            th { {t!("clients-on-hold-timeout")} }
                            th { {t!("clients-on-hold-duration")} }
                            th { {t!("common-expiry")} }
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
                                let user_id = c.get("user_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                let used = c.get("traffic_used_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                                let limit = c.get("traffic_limit_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                                let all_time = c.get("all_time_used_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                                let is_exceeded = c.get("is_exceeded").and_then(|v| v.as_bool()).unwrap_or(false);
                                let expiry = c.get("expiry_date").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let strategy = c.get("data_limit_reset_strategy").and_then(|v| v.as_str()).unwrap_or("no_reset").to_string();
                                let on_hold_timeout = c.get("on_hold_timeout").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let on_hold_duration_secs = c.get("on_hold_expire_duration_secs").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_default();
                                let group_ids = c.get("group_ids").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                let group_names: Vec<String> = group_ids.iter()
                                    .filter_map(|gid| gid.as_str())
                                    .filter_map(|gid| {
                                        groups_data.iter().find(|g| {
                                            g.get("id").and_then(|v| v.as_str()) == Some(gid)
                                        }).and_then(|g| g.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                                    })
                                    .collect();
                                let did = id.clone();
                                let edit_id_clone = id.clone();
                                let edit_data = c.clone();
                                rsx! {
                                    tr {
                                        td { "{name}" }
                                        td { "{email}" }
                                        td { class: "mono", "{user_id}" }
                                        td {
                                            StatusBadge { status: status.clone() }
                                        }
                                        td { "{format_bytes(used)}" }
                                        td { "{format_bytes(limit)}" }
                                        td { "{format_bytes(all_time)}" }
                                        td {
                                            if is_exceeded {
                                                span { class: "badge danger", {t!("clients-is-exceeded")} }
                                            } else {
                                                "-"
                                            }
                                        }
                                        td {
                                            if status == "on_hold" {
                                                span { class: "badge warning", {t!("clients-on-hold")} }
                                            } else {
                                                "-"
                                            }
                                        }
                                        td {
                                            if on_hold_timeout.is_empty() {
                                                "-"
                                            } else {
                                                "{on_hold_timeout}"
                                            }
                                        }
                                        td {
                                            if on_hold_duration_secs.is_empty() {
                                                "-"
                                            } else {
                                                "{on_hold_duration_secs} s"
                                            }
                                        }
                                        td { "{expiry}" }
                                        td {
                                            match strategy.as_str() {
                                                "no_reset" => t!("strategy-no-reset").to_string(),
                                                "daily" => t!("strategy-daily").to_string(),
                                                "weekly" => t!("strategy-weekly").to_string(),
                                                "monthly" => t!("strategy-monthly").to_string(),
                                                "yearly" => t!("strategy-yearly").to_string(),
                                                _ => strategy.clone(),
                                            }
                                        }
                                        td {
                                            { if group_names.is_empty() { "-".to_string() } else { group_names.join(", ") } }
                                        }
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
                let email = new_email.read().clone();
                let user_id = new_user_id.read().clone();
                let limit = new_limit.read().parse::<i64>().unwrap_or(0);
                let reset_day = new_reset_day.read().parse::<i32>().ok();
                let strategy = new_reset_strategy.read().clone();
                let max_devices = new_max_devices.read().parse::<i32>().ok();
                let expiry = new_expiry.read().clone();
                let status = new_status.read().clone();
                let on_hold_duration = new_on_hold_duration.read().clone();
                let on_hold_timeout = new_on_hold_timeout.read().clone();
                let group_ids = selected_group_ids.read().clone();

                if *is_edit.read() {
                    let id = edit_id.read().clone();
                    spawn(async move {
                        let mut payload = json!({
                            "name": name,
                            "traffic_limit_bytes": limit,
                            "data_limit_reset_strategy": strategy,
                            "group_ids": group_ids,
                            "status": status,
                        });
                        let email_opt = if email.is_empty() { None } else { Some(email) };
                        if let Some(e) = email_opt {
                            payload["email"] = json!(e);
                        }
                        if !user_id.is_empty() {
                            payload["user_id"] = json!(user_id);
                        }
                        if let Some(rd) = reset_day {
                            payload["reset_day"] = json!(rd);
                        }
                        if let Some(md) = max_devices {
                            payload["max_devices"] = json!(md);
                        }
                        if !expiry.is_empty() {
                            payload["expiry_date"] = json!(expiry);
                        }
                        if let Ok(d) = on_hold_duration.parse::<i64>() {
                            payload["on_hold_expire_duration_secs"] = json!(d);
                        }
                        if !on_hold_timeout.is_empty() {
                            payload["on_hold_timeout"] = json!(on_hold_timeout);
                        }
                        match api::update_client(&id, payload).await {
                            Ok(_) => {
                                reset_form();
                                is_edit.set(false);
                                show_modal.set(false);
                                clients.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                } else {
                    spawn(async move {
                        let email_opt = if email.is_empty() { None } else { Some(email.as_str()) };
                        let mut payload = json!({
                            "name": name,
                            "traffic_limit_bytes": limit,
                            "data_limit_reset_strategy": strategy,
                            "group_ids": group_ids,
                        });
                        if let Some(e) = email_opt {
                            payload["email"] = json!(e);
                        }
                        if !user_id.is_empty() {
                            payload["user_id"] = json!(user_id);
                        }
                        if let Some(rd) = reset_day {
                            payload["reset_day"] = json!(rd);
                        }
                        if let Some(md) = max_devices {
                            payload["max_devices"] = json!(md);
                        }
                        if !expiry.is_empty() {
                            payload["expiry_date"] = json!(expiry);
                        }
                        if let Ok(d) = on_hold_duration.parse::<i64>() {
                            payload["on_hold_expire_duration_secs"] = json!(d);
                        }
                        if !on_hold_timeout.is_empty() {
                            payload["on_hold_timeout"] = json!(on_hold_timeout);
                        }
                        match api::create_client_from_payload(payload).await {
                            Ok(_) => {
                                reset_form();
                                show_modal.set(false);
                                clients.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                }
            },
            confirm_text: Some(confirm_text),
            FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("user01".to_string()), input_type: None, error: None }
            FormInput { label: t!("clients-email").to_string(), value: new_email, placeholder: Some("user@example.com".to_string()), input_type: Some("email".to_string()), error: None }
            FormInput { label: t!("clients-user-id").to_string(), value: new_user_id, placeholder: Some("UUID".to_string()), input_type: None, error: None }
            FormInput { label: format!("{} (bytes)", t!("clients-traffic-limit")), value: new_limit, placeholder: Some("1073741824".to_string()), input_type: Some("number".to_string()), error: None }
            FormDate { label: t!("clients-expiry-date").to_string(), value: new_expiry, error: None }
            FormInput { label: format!("{} (1-31)", t!("clients-reset-day")), value: new_reset_day, placeholder: Some("1".to_string()), input_type: Some("number".to_string()), error: None }
            FormInput { label: t!("clients-max-devices").to_string(), value: new_max_devices, placeholder: Some("3".to_string()), input_type: Some("number".to_string()), error: None }
            FormSelect {
                label: t!("clients-reset-strategy").to_string(),
                value: new_reset_strategy,
                options: vec![
                    ("no_reset".to_string(), t!("strategy-no-reset").to_string()),
                    ("daily".to_string(), t!("strategy-daily").to_string()),
                    ("weekly".to_string(), t!("strategy-weekly").to_string()),
                    ("monthly".to_string(), t!("strategy-monthly").to_string()),
                    ("yearly".to_string(), t!("strategy-yearly").to_string()),
                ],
                error: None,
            }
            if *is_edit.read() {
                FormSelect {
                    label: t!("common-status").to_string(),
                    value: new_status,
                    options: vec![
                        ("active".to_string(), t!("common-active").to_string()),
                        ("inactive".to_string(), t!("common-disabled").to_string()),
                        ("on_hold".to_string(), t!("clients-on-hold").to_string()),
                    ],
                    error: None,
                }
            }
            FormInput { label: format!("{} (s)", t!("clients-on-hold-duration")), value: new_on_hold_duration, placeholder: Some("86400".to_string()), input_type: Some("number".to_string()), error: None }
            FormInput { label: t!("clients-on-hold-timeout").to_string(), value: new_on_hold_timeout, placeholder: Some("2025-01-01T00:00:00+00:00".to_string()), input_type: Some("datetime-local".to_string()), error: None }
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
                                        checked: selected.read().contains(&checkbox_gid),
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

        ConfirmDialog {
            title: t!("clients-delete-title").to_string(),
            message: t!("clients-delete-confirm").to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_client(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            clients.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::{Value, json};

use crate::api;
use crate::components::{Alert, ConfirmDialog, FormInput, FormTextarea, Modal, Pagination, SearchInput, StatusBadge};

fn parse_json_object(s: &str) -> Result<Value, String> {
    if s.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(s).map_err(|e| format!("Invalid JSON: {}", e))
}

#[component]
pub fn Nodes() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 20u64);
    let search_query = use_signal(String::new);
    let mut nodes = use_resource(move || async move {
        api::get_nodes_paginated(*page.read(), *per_page.read())
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
    let mut created_token = use_signal(|| None::<String>);

    // Create form
    let mut new_name = use_signal(String::new);
    let mut new_hostname = use_signal(String::new);
    let mut new_address = use_signal(String::new);
    let mut new_usage_coefficient = use_signal(|| "1.0".to_string());
    let mut new_labels = use_signal(|| "{}".to_string());
    let mut new_parent_id = use_signal(String::new);
    let mut selected_group_ids = use_signal(Vec::<String>::new);

    // Edit form
    let mut edit_name = use_signal(String::new);
    let mut edit_hostname = use_signal(String::new);
    let mut edit_address = use_signal(String::new);
    let mut edit_usage_coefficient = use_signal(|| "1.0".to_string());
    let mut edit_labels = use_signal(|| "{}".to_string());
    let mut edit_parent_id = use_signal(String::new);
    let mut edit_clear_parent = use_signal(|| false);
    let mut edit_selected_group_ids = use_signal(Vec::<String>::new);

    let nodes_data = nodes
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = nodes
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let groups_data = groups
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut reset_form = move || {
        new_name.set(String::new());
        new_hostname.set(String::new());
        new_address.set(String::new());
        new_usage_coefficient.set("1.0".to_string());
        new_labels.set("{}".to_string());
        new_parent_id.set(String::new());
        selected_group_ids.set(Vec::new());
        created_token.set(None);
    };

    let mut load_edit_form = move |node: &Value| {
        edit_name.set(node.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string());
        edit_hostname.set(node.get("hostname").and_then(|v| v.as_str()).unwrap_or("").to_string());
        edit_address.set(node.get("address").and_then(|v| v.as_str()).unwrap_or("").to_string());
        edit_usage_coefficient.set(
            node.get("usage_coefficient")
                .and_then(|v| v.as_f64())
                .map(|v| v.to_string())
                .unwrap_or_else(|| "1.0".to_string()),
        );
        if let Some(labels) = node.get("labels").and_then(|v| v.as_object()) {
            edit_labels.set(serde_json::to_string_pretty(labels).unwrap_or_else(|_| "{}".to_string()));
        } else {
            edit_labels.set("{}".to_string());
        }
        let gids: Vec<String> = node
            .get("group_ids")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        edit_selected_group_ids.set(gids);
        let pid = node.get("parent_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        edit_parent_id.set(pid);
        edit_clear_parent.set(false);
    };

    let modal_title = if *is_edit.read() {
        t!("nodes-edit-title").to_string()
    } else {
        t!("nodes-create-title").to_string()
    };
    let confirm_text = if *is_edit.read() {
        t!("common-update").to_string()
    } else {
        t!("common-create").to_string()
    };

    // Filter nodes based on search query
    let query = search_query.read().to_lowercase();
    let filtered_nodes: Vec<Value> = if query.is_empty() {
        nodes_data.clone()
    } else {
        nodes_data.iter().filter(|n| {
            let name = n.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let hostname = n.get("hostname").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let address = n.get("address").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            name.contains(&query) || hostname.contains(&query) || address.contains(&query)
        }).cloned().collect()
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("nodes-title")} }
                div { class: "page-header-actions",
                    SearchInput {
                        placeholder: Some("Search nodes...".to_string()),
                        value: search_query,
                    }
                    button {
                        onclick: move |_| {
                            reset_form();
                            error.set(None);
                            is_edit.set(false);
                            show_modal.set(true);
                        },
                        {t!("nodes-create")}
                    }
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if filtered_nodes.is_empty() {
                div { class: "empty-state", p { "No nodes found." } }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("common-name")} }
                            th { {t!("node-hostname")} }
                            th { {t!("node-address")} }
                            th { {t!("common-status")} }
                            th { {t!("nodes-parent-id")} }
                            th { {t!("node-cores-available")} }
                            th { {t!("node-usage-coefficient")} }
                            th { {t!("common-labels")} }
                            th { {t!("nodes-groups")} }
                            th { {t!("common-actions")} }
                        }
                    }
                    tbody {
                        for node in filtered_nodes.iter() {
                            {
                                let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let hostname = node.get("hostname").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let address = node.get("address").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let status = node.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                let cores = node.get("cores_available").cloned().unwrap_or(json!([]));
                                let parent_id = node.get("parent_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let coeff = node.get("usage_coefficient").and_then(|v| v.as_f64()).unwrap_or(1.0);
                                let labels = node.get("labels").cloned().unwrap_or(json!({}));
                                let group_ids = node.get("group_ids").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                let group_names: Vec<String> = group_ids.iter()
                                    .filter_map(|gid| gid.as_str())
                                    .filter_map(|gid| {
                                        groups_data.iter().find(|g| {
                                            g.get("id").and_then(|v| v.as_str()) == Some(gid)
                                        }).and_then(|g| g.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                                    })
                                    .collect();
                                let push_id = id.clone();
                                let did = id.clone();
                                let edit_id_clone = id.clone();
                                let edit_data = node.clone();
                                rsx! {
                                    tr {
                                        td { "{name}" }
                                        td { "{hostname}" }
                                        td { "{address}" }
                                        td {
                                            StatusBadge { status: status.clone() }
                                        }
                                        td {
                                            if parent_id.is_empty() {
                                                "-"
                                            } else {
                                                span { {t!("nodes-child-of")} " " span { class: "mono", "{parent_id}" } }
                                            }
                                        }
                                        td { class: "mono", "{cores}" }
                                        td { "{coeff}" }
                                        td { class: "mono", "{labels}" }
                                        td {
                                            if group_names.is_empty() {
                                                "-"
                                            } else {
                                                "{group_names.join(\", \")}"
                                            }
                                        }
                                        td {
                                            button {
                                                onclick: move |_| {
                                                    load_edit_form(&edit_data);
                                                    edit_id.set(edit_id_clone.clone());
                                                    error.set(None);
                                                    is_edit.set(true);
                                                    show_modal.set(true);
                                                },
                                                {t!("common-edit")}
                                            }
                                            button {
                                                onclick: move |_| {
                                                    let id = push_id.clone();
                                                    spawn(async move {
                                                        let payload = json!({ "core_type": "sing-box", "restart": true, "version": "1" });
                                                        if let Err(e) = api::push_config(&id, payload).await {
                                                            error.set(Some(e.to_string()));
                                                        }
                                                    });
                                                },
                                                {t!("nodes-push-config")}
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
                if *is_edit.read() {
                    let id = edit_id.read().clone();
                    let name = edit_name.read().clone();
                    if name.is_empty() {
                        error.set(Some(t!("common-required-field").to_string()));
                        return;
                    }
                    let hostname = edit_hostname.read().clone();
                    let address = edit_address.read().clone();
                    let usage_coefficient = edit_usage_coefficient.read().parse::<f64>().unwrap_or(1.0);
                    let labels = match parse_json_object(&edit_labels.read()) {
                        Ok(v) => v,
                        Err(e) => { error.set(Some(e)); return; }
                    };
                    let group_ids = edit_selected_group_ids.read().clone();
                    let parent_id_input = edit_parent_id.read().clone();
                    let clear_parent = *edit_clear_parent.read();

                    spawn(async move {
                        let mut payload = json!({
                            "name": name,
                            "hostname": hostname,
                            "address": address,
                            "usage_coefficient": usage_coefficient,
                            "labels": labels,
                            "group_ids": group_ids,
                        });
                        if clear_parent {
                            payload["parent_id"] = json!(null);
                        } else if !parent_id_input.is_empty() {
                            payload["parent_id"] = json!(parent_id_input);
                        }
                        match api::update_node(&id, payload).await {
                            Ok(_) => {
                                error.set(None);
                                is_edit.set(false);
                                show_modal.set(false);
                                nodes.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                } else {
                    let name = new_name.read().clone();
                    if name.is_empty() {
                        error.set(Some(t!("common-required-field").to_string()));
                        return;
                    }
                    let hostname = new_hostname.read().clone();
                    let address = new_address.read().clone();
                    let usage_coefficient = new_usage_coefficient.read().parse::<f64>().unwrap_or(1.0);
                    let labels = match parse_json_object(&new_labels.read()) {
                        Ok(v) => v,
                        Err(e) => { error.set(Some(e)); return; }
                    };
                    let group_ids = selected_group_ids.read().clone();
                    let parent_id_input = new_parent_id.read().clone();
                    let parent_id_opt: Option<String> = if parent_id_input.is_empty() { None } else { Some(parent_id_input) };
                    spawn(async move {
                        let parent_id_ref = parent_id_opt.as_deref();
                        match api::create_node(
                            &name, &hostname, &address, usage_coefficient, labels, group_ids, parent_id_ref
                        ).await {
                            Ok(resp) => {
                                if let Some(token) = resp.get("data")
                                    .and_then(|d| d.get("token"))
                                    .and_then(|v| v.as_str())
                                {
                                    created_token.set(Some(token.to_string()));
                                }
                                nodes.restart();
                            }
                            Err(e) => error.set(Some(e.to_string())),
                        }
                    });
                }
            },
            confirm_text: Some(confirm_text),

            if *is_edit.read() {
                FormInput { label: t!("common-name").to_string(), value: edit_name, placeholder: Some("node-01".to_string()), input_type: None, error: None }
                FormInput { label: t!("node-hostname").to_string(), value: edit_hostname, placeholder: Some("host.example.com".to_string()), input_type: None, error: None }
                FormInput { label: t!("node-address").to_string(), value: edit_address, placeholder: Some("192.168.1.1".to_string()), input_type: None, error: None }
                FormInput { label: t!("node-usage-coefficient").to_string(), value: edit_usage_coefficient, placeholder: Some("1.0".to_string()), input_type: Some("number".to_string()), error: None }
                FormInput { label: t!("nodes-parent-id").to_string(), value: edit_parent_id, placeholder: Some("00000000-0000-0000-0000-000000000000".to_string()), input_type: None, error: None }
                div { class: "form-group",
                    label { class: "checkbox-label",
                        input {
                            r#type: "checkbox",
                            checked: "{edit_clear_parent.read().clone()}",
                            onchange: move |e| edit_clear_parent.set(e.checked()),
                        }
                        " "
                        {t!("nodes-clear-parent")}
                    }
                }
                FormTextarea { label: t!("common-labels").to_string(), value: edit_labels, placeholder: Some("{\"region\": \"eu\"}".to_string()), rows: Some(3), error: None }
                div { class: "form-group",
                    label { {t!("nodes-groups")} }
                    div { class: "checkbox-group",
                        for g in groups_data.iter() {
                            {
                                let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let gname = g.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let checkbox_gid = gid.clone();
                                let mut selected = edit_selected_group_ids;
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
            } else {
                FormInput { label: t!("common-name").to_string(), value: new_name, placeholder: Some("node-01".to_string()), input_type: None, error: None }
                FormInput { label: t!("node-hostname").to_string(), value: new_hostname, placeholder: Some("host.example.com".to_string()), input_type: None, error: None }
                FormInput { label: t!("node-address").to_string(), value: new_address, placeholder: Some("192.168.1.1".to_string()), input_type: None, error: None }
                FormInput { label: t!("node-usage-coefficient").to_string(), value: new_usage_coefficient, placeholder: Some("1.0".to_string()), input_type: Some("number".to_string()), error: None }
                FormInput { label: t!("nodes-parent-id").to_string(), value: new_parent_id, placeholder: Some("00000000-0000-0000-0000-000000000000".to_string()), input_type: None, error: None }
                FormTextarea { label: t!("common-labels").to_string(), value: new_labels, placeholder: Some("{\"region\": \"eu\"}".to_string()), rows: Some(3), error: None }
                div { class: "form-group",
                    label { {t!("nodes-groups")} }
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
                if let Some(token) = created_token.read().as_ref() {
                    Alert { level: "warning".to_string(),
                        p { {t!("nodes-token-warning")} }
                        pre { class: "mono secret-box", "{token}" }
                    }
                }
            }
        }

        ConfirmDialog {
            title: t!("common-confirm-delete").to_string(),
            message: "Delete this node?".to_string(),
            show: show_delete,
            on_confirm: move |_| {
                let id = delete_id.read().clone();
                if !id.is_empty() {
                    spawn(async move {
                        if let Err(e) = api::delete_node(&id).await {
                            error.set(Some(e.to_string()));
                        } else {
                            nodes.restart();
                        }
                    });
                }
            },
            confirm_text: Some(t!("common-delete").to_string()),
        }
    }
}

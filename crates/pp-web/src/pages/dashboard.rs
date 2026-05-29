use dioxus::prelude::*;
use dioxus_i18n::t;
use serde_json::Value;

use crate::api;

fn extract_array(res: &Option<Value>, key: &str) -> Vec<Value> {
    res.as_ref()
        .and_then(|r| r.get(key))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

#[component]
pub fn Dashboard() -> Element {
    let mut nodes = use_resource(|| async move { api::get_nodes().await.unwrap_or_default() });
    let mut protocols = use_resource(|| async move { api::get_protocols(1, 100).await.unwrap_or_default() });
    let mut clients = use_resource(|| async move { api::get_clients().await.unwrap_or_default() });
    let mut bindings = use_resource(|| async move { api::get_bindings().await.unwrap_or_default() });
    let mut metrics = use_resource(|| async move { api::get_metrics().await.unwrap_or_default() });
    let mut logs = use_resource(|| async move { api::get_logs().await.unwrap_or_default() });

    let nodes_data = extract_array(&*nodes.read(), "data");
    let protocols_data = extract_array(&*protocols.read(), "data");
    let clients_data = extract_array(&*clients.read(), "data");
    let bindings_data = extract_array(&*bindings.read(), "data");
    let metrics_data = extract_array(&*metrics.read(), "data");
    let logs_data = extract_array(&*logs.read(), "data");

    let online_count = nodes_data.iter().filter(|n| {
        n.get("status").and_then(|v| v.as_str()).unwrap_or("") == "online"
    }).count();

    rsx! {
        div { class: "dashboard",
            h1 { {t!("dashboard-title")} }
            div { class: "stats-grid",
                div { class: "stat-card",
                    h3 { {t!("dashboard-total-nodes")} }
                    p { "{nodes_data.len()}" }
                }
                div { class: "stat-card",
                    h3 { {t!("dashboard-online")} }
                    p { "{online_count}" }
                }
                div { class: "stat-card",
                    h3 { {t!("dashboard-protocols")} }
                    p { "{protocols_data.len()}" }
                }
                div { class: "stat-card",
                    h3 { {t!("dashboard-clients")} }
                    p { "{clients_data.len()}" }
                }
                div { class: "stat-card",
                    h3 { {t!("dashboard-bindings")} }
                    p { "{bindings_data.len()}" }
                }
                div { class: "stat-card",
                    h3 { {t!("dashboard-metrics-records")} }
                    p { "{metrics_data.len()}" }
                }
            }

            h2 { {t!("dashboard-recent-logs")} }
            table { class: "data-table",
                thead {
                    tr {
                        th { {t!("log-level")} }
                        th { {t!("log-source")} }
                        th { {t!("log-message")} }
                        th { {t!("log-time")} }
                    }
                }
                tbody {
                    for log in logs_data.iter().take(5) {
                        {
                            let level = log.get("level").and_then(|v| v.as_str()).unwrap_or("-");
                            let source = log.get("source").and_then(|v| v.as_str()).unwrap_or("-");
                            let message = log.get("message").and_then(|v| v.as_str()).unwrap_or("-");
                            let created_at = log.get("created_at").and_then(|v| v.as_str()).unwrap_or("-");
                            let level_class = format!("level-{level}");
                            rsx! {
                                tr {
                                    td { class: "{level_class}", "{level}" }
                                    td { "{source}" }
                                    td { "{message}" }
                                    td { "{created_at}" }
                                }
                            }
                        }
                    }
                }
            }

            h2 { {t!("dashboard-node-status")} }
            table { class: "data-table",
                thead {
                    tr {
                        th { {t!("common-name")} }
                        th { {t!("node-hostname")} }
                        th { {t!("node-address")} }
                        th { {t!("common-status")} }
                        th { {t!("node-last-seen")} }
                    }
                }
                tbody {
                    for node in nodes_data.iter() {
                        {
                            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                            let hostname = node.get("hostname").and_then(|v| v.as_str()).unwrap_or("-");
                            let address = node.get("address").and_then(|v| v.as_str()).unwrap_or("-");
                            let status = node.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let last_seen = node.get("last_seen_at").and_then(|v| v.as_str()).unwrap_or("never");
                            let status_class = format!("status-{status}");
                            rsx! {
                                tr {
                                    td { "{name}" }
                                    td { "{hostname}" }
                                    td { "{address}" }
                                    td { class: "{status_class}", "{status}" }
                                    td { "{last_seen}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

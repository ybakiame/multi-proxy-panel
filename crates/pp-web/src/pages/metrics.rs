use dioxus::prelude::*;
use serde_json::Value;

use crate::api;
use crate::components::FormSelect;

#[component]
pub fn Metrics() -> Element {
    let mut nodes = use_resource(|| async move { api::get_nodes().await.unwrap_or_default() });
    let mut selected_node = use_signal(|| String::new());

    let mut metrics = use_resource(move || {
        let node_id = selected_node.read().clone();
        async move {
            if node_id.is_empty() {
                api::get_metrics().await.unwrap_or_default()
            } else {
                api::get_metrics_for_node(&node_id).await.unwrap_or_default()
            }
        }
    });

    let node_options: Vec<(String, String)> = std::iter::once(("".to_string(), "All Nodes".to_string()))
        .chain(
            nodes.read()
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
        )
        .collect();

    let metrics_data = metrics.read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { "Host Metrics" }
                div { class: "filters",
                    FormSelect {
                        label: "Filter by Node".to_string(),
                        value: selected_node,
                        options: node_options,
                    }
                    button {
                        onclick: move |_| metrics.restart(),
                        "Refresh"
                    }
                }
            }

            table { class: "data-table",
                thead {
                    tr {
                        th { "Node ID" }
                        th { "CPU %" }
                        th { "Memory" }
                        th { "Disk" }
                        th { "Load Avg" }
                        th { "Timestamp" }
                    }
                }
                tbody {
                    for m in metrics_data.iter().take(100) {
                        {
                            let node_id = m.get("node_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let cpu = m.get("cpu_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let mem_used = m.get("mem_used").and_then(|v| v.as_i64()).unwrap_or(0);
                            let mem_total = m.get("mem_total").and_then(|v| v.as_i64()).unwrap_or(0);
                            let disk_used = m.get("disk_used").and_then(|v| v.as_i64()).unwrap_or(0);
                            let disk_total = m.get("disk_total").and_then(|v| v.as_i64()).unwrap_or(0);
                            let timestamp = m.get("timestamp").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                            let load_str = if let Some(load) = m.get("load_avg").and_then(|v| v.as_array()) {
                                load.iter().map(|v| v.as_f64().unwrap_or(0.0)).map(|f| format!("{:.2}", f)).collect::<Vec<_>>().join(", ")
                            } else {
                                let l1 = m.get("load_avg1").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let l5 = m.get("load_avg5").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let l15 = m.get("load_avg_15").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                format!("{:.2}, {:.2}, {:.2}", l1, l5, l15)
                            };
                            rsx! {
                                tr {
                                    td { class: "mono", "{node_id}" }
                                    td { "{cpu:.1}%" }
                                    td { "{mem_used} / {mem_total}" }
                                    td { "{disk_used} / {disk_total}" }
                                    td { "{load_str}" }
                                    td { "{timestamp}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

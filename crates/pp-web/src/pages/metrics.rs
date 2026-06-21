use dioxus::prelude::*;
use dioxus_i18n::t;
use gloo_timers::future::TimeoutFuture;

use crate::api;
use crate::components::{Alert, EmptyState, FormSelect};

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
pub fn Metrics() -> Element {
    let nodes = use_resource(|| async move { api::get_nodes().await.unwrap_or_default() });
    let selected_node = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut auto_refresh = use_signal(|| false);

    let mut metrics = use_resource(move || {
        let node_id = selected_node.read().clone();
        async move {
            let res = if node_id.is_empty() {
                api::get_metrics().await
            } else {
                api::get_metrics_for_node(&node_id).await
            };
            match res {
                Ok(v) => v,
                Err(e) => {
                    error.set(Some(e.to_string()));
                    serde_json::Value::default()
                }
            }
        }
    });

    use_future(move || {
        let enabled = *auto_refresh.read();
        async move {
            if !enabled {
                return;
            }
            loop {
                TimeoutFuture::new(30_000).await;
                if !*auto_refresh.read() {
                    break;
                }
                metrics.restart();
            }
        }
    });

    let node_options: Vec<(String, String)> =
        std::iter::once(("".to_string(), t!("common-all").to_string()))
            .chain(
                nodes
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
                    }),
            )
            .collect();

    let metrics_data = metrics
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("metrics-title")} }
                div { class: "filters",
                    FormSelect {
                        label: t!("metrics-filter-by-node").to_string(),
                        value: selected_node,
                        options: node_options,
                        error: None,
                    }
                    div { class: "form-group inline",
                        label {
                            input {
                                r#type: "checkbox",
                                checked: "{auto_refresh.read().clone()}",
                                onchange: move |e| auto_refresh.set(e.checked()),
                            }
                            " "
                            {t!("metrics-auto-refresh")}
                        }
                    }
                    button {
                        onclick: move |_| {
                            error.set(None);
                            metrics.restart();
                        },
                        {t!("common-refresh")}
                    }
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if metrics_data.is_empty() {
                EmptyState { message: t!("metrics-empty").to_string() }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("node-address")} }
                            th { {t!("metrics-cpu")} }
                            th { {t!("metrics-memory")} }
                            th { {t!("metrics-disk")} }
                            th { {t!("metrics-load-avg")} }
                            th { {t!("metrics-timestamp")} }
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
                                        td { "{format_bytes(mem_used)} / {format_bytes(mem_total)}" }
                                        td { "{format_bytes(disk_used)} / {format_bytes(disk_total)}" }
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
}

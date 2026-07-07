use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api;
use crate::components::{Alert, EmptyState, FormSelect, Pagination};

#[component]
pub fn Logs() -> Element {
    let mut page = use_signal(|| 1u64);
    let mut per_page = use_signal(|| 50u64);
    let level_filter = use_signal(String::new);
    let mut source_filter = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let mut logs = use_resource(move || {
        let level = level_filter.read().clone();
        let source = source_filter.read().clone();
        let page = *page.read();
        let per_page = *per_page.read();
        async move {
            let level_opt = if level.is_empty() { None } else { Some(level) };
            let source_opt = if source.is_empty() {
                None
            } else {
                Some(source)
            };
            api::get_logs_filtered(level_opt, source_opt, page, per_page)
                .await
                .unwrap_or_default()
        }
    });

    let logs_data = logs
        .read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total = logs
        .read()
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("nav-logs")} }
                div { class: "filters",
                    FormSelect {
                        label: t!("log-level").to_string(),
                        value: level_filter,
                        options: vec![
                            ("".to_string(), t!("common-all").to_string()),
                            ("info".to_string(), "Info".to_string()),
                            ("warn".to_string(), "Warn".to_string()),
                            ("error".to_string(), "Error".to_string()),
                            ("debug".to_string(), "Debug".to_string()),
                        ],
                        error: None,
                    }
                    div { class: "form-group",
                        label { {t!("log-source")} }
                        input {
                            r#type: "text",
                            placeholder: "Filter by source...",
                            value: "{source_filter.read().clone()}",
                            oninput: move |e| source_filter.set(e.value()),
                        }
                    }
                    button {
                        onclick: move |_| {
                            error.set(None);
                            logs.restart();
                        },
                        {t!("common-filter")}
                    }
                }
            }

            if let Some(err) = error.read().as_ref() {
                Alert { level: "error".to_string(), p { "{err}" } }
            }

            if logs_data.is_empty() {
                EmptyState { message: t!("logs-empty").to_string() }
            } else {
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
                        for log in logs_data.iter() {
                            {
                                let level = log.get("level").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let source = log.get("source").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let message = log.get("message").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let created_at = log.get("created_at").and_then(|v| v.as_str()).unwrap_or("-").to_string();
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
    }
}

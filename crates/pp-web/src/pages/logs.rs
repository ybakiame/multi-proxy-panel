use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api;
use crate::components::FormSelect;

#[component]
pub fn Logs() -> Element {
    let mut level_filter = use_signal(|| String::new());
    let mut source_filter = use_signal(|| String::new());

    let mut logs = use_resource(move || {
        let level = level_filter.read().clone();
        let source = source_filter.read().clone();
        async move {
            let level_opt = if level.is_empty() { None } else { Some(level) };
            let source_opt = if source.is_empty() { None } else { Some(source) };
            api::get_logs_filtered(level_opt, source_opt, 100).await.unwrap_or_default()
        }
    });

    let logs_data = logs.read()
        .as_ref()
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

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
                        onclick: move |_| logs.restart(),
                        {t!("common-filter")}
                    }
                }
            }

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
                    for log in logs_data.iter().take(100) {
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
    }
}

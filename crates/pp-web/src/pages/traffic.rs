use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api;
use crate::components::{Alert, EmptyState};

#[component]
pub fn Traffic() -> Element {
    let traffic = use_resource(|| async move { api::get_traffic(None, None).await });

    let records = traffic
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|r| r.get("data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                h1 { {t!("traffic-title")} }
            }

            if traffic.read().as_ref().map(|r| r.is_err()).unwrap_or(false) {
                Alert { level: "error".to_string(), p { "Failed to load traffic records" } }
            }

            if records.is_empty() {
                EmptyState { message: t!("traffic-empty").to_string() }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("traffic-hour")} }
                            th { {t!("traffic-node")} }
                            th { {t!("traffic-client")} }
                            th { {t!("traffic-upload")} }
                            th { {t!("traffic-download")} }
                            th { {t!("traffic-total")} }
                        }
                    }
                    tbody {
                        for r in records.iter() {
                            {
                                let hour = r.get("hour_bucket").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let node_id = r.get("node_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let client_id = r.get("client_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let up = r.get("upload_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                                let down = r.get("download_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                                let total = up + down;
                                rsx! {
                                    tr {
                                        td { "{hour}" }
                                        td { class: "mono", "{node_id}" }
                                        td { class: "mono", "{client_id}" }
                                        td { "{up}" }
                                        td { "{down}" }
                                        td { "{total}" }
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

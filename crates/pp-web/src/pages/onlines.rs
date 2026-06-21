use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api;
use crate::components::{Alert, EmptyState};

#[component]
pub fn Onlines() -> Element {
    let onlines = use_resource(|| async move { api::get_onlines(None, None).await });

    let sessions = onlines
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
                h1 { {t!("onlines-title")} }
            }

            if onlines.read().as_ref().map(|r| r.is_err()).unwrap_or(false) {
                Alert { level: "error".to_string(), p { "Failed to load online sessions" } }
            }

            if sessions.is_empty() {
                EmptyState { message: t!("onlines-empty").to_string() }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { {t!("onlines-client")} }
                            th { {t!("onlines-node")} }
                            th { {t!("onlines-ip")} }
                            th { {t!("onlines-inbound")} }
                            th { {t!("onlines-connected-at")} }
                            th { {t!("onlines-last-active")} }
                        }
                    }
                    tbody {
                        for s in sessions.iter() {
                            {
                                let client_id = s.get("client_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let node_id = s.get("node_id").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let ip = s.get("ip_address").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let inbound = s.get("inbound_tag").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let connected = s.get("connected_at").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                let last_active = s.get("last_active_at").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                                rsx! {
                                    tr {
                                        td { class: "mono", "{client_id}" }
                                        td { class: "mono", "{node_id}" }
                                        td { "{ip}" }
                                        td { "{inbound}" }
                                        td { "{connected}" }
                                        td { "{last_active}" }
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

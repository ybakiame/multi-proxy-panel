use dioxus::prelude::*;

#[component]
pub fn StatusBadge(status: String) -> Element {
    let normalized = status.to_lowercase();
    let class = match normalized.as_str() {
        "online" | "active" => "status-online",
        "offline" | "inactive" => "status-offline",
        "degraded" => "status-degraded",
        "connecting" => "status-connecting",
        _ => "status-unknown",
    };
    rsx! {
        span { class: "status-badge {class}", role: "status", aria_label: "{status}", "{status}" }
    }
}

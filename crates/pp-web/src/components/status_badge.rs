use dioxus::prelude::*;

#[component]
pub fn StatusBadge(status: String) -> Element {
    let class = match status.as_str() {
        "online" | "active" => "status-online",
        "offline" | "inactive" => "status-offline",
        "degraded" => "status-degraded",
        "connecting" => "status-connecting",
        _ => "",
    };
    rsx! {
        span { class: "{class}", "{status}" }
    }
}

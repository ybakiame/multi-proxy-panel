use dioxus::prelude::*;

#[component]
pub fn Alert(level: String, children: Element) -> Element {
    let class = format!("alert alert-{}", level);
    rsx! {
        div { class: "{class}", role: "alert", {children} }
    }
}

#[component]
pub fn Loading() -> Element {
    rsx! {
        div { class: "loading-container",
            div { class: "spinner", aria_hidden: "true" }
            p { "Loading..." }
        }
    }
}

#[component]
pub fn EmptyState(message: String, children: Option<Element>) -> Element {
    rsx! {
        div { class: "empty-state",
            span { class: "empty-state-icon", "📭" }
            p { "{message}" }
            if let Some(actions) = children {
                {actions}
            }
        }
    }
}

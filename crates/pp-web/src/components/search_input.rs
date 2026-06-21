use dioxus::prelude::*;

/// A search input component for filtering data tables.
#[component]
pub fn SearchInput(
    placeholder: Option<String>,
    value: Signal<String>,
    on_search: Option<EventHandler<String>>,
) -> Element {
    let placeholder = placeholder.unwrap_or_else(|| "Search...".to_string());

    rsx! {
        div { class: "search-input-wrapper",
            input {
                class: "search-input",
                r#type: "text",
                placeholder: "{placeholder}",
                value: "{value.read().clone()}",
                oninput: move |e| {
                    let val = e.value();
                    value.set(val.clone());
                    if let Some(handler) = &on_search {
                        handler.call(val);
                    }
                },
            }
            if value.read().is_empty() {
                span { class: "search-icon", "🔍" }
            } else {
                button {
                    class: "search-clear",
                    onclick: move |_| {
                        value.set(String::new());
                        if let Some(handler) = &on_search {
                            handler.call(String::new());
                        }
                    },
                    "×"
                }
            }
        }
    }
}

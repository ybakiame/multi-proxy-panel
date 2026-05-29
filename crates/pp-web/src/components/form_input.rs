use dioxus::prelude::*;

#[component]
pub fn FormInput(
    label: String,
    value: Signal<String>,
    placeholder: Option<String>,
    input_type: Option<String>,
) -> Element {
    let placeholder = placeholder.unwrap_or_default();
    let input_type = input_type.unwrap_or_else(|| "text".to_string());
    rsx! {
        div { class: "form-group",
            label { "{label}" }
            input {
                r#type: "{input_type}",
                placeholder: "{placeholder}",
                value: "{value.read().clone()}",
                oninput: move |e| value.set(e.value()),
            }
        }
    }
}

#[component]
pub fn FormSelect(
    label: String,
    value: Signal<String>,
    options: Vec<(String, String)>,
) -> Element {
    rsx! {
        div { class: "form-group",
            label { "{label}" }
            select {
                value: "{value.read().clone()}",
                onchange: move |e| value.set(e.value()),
                for (key , text) in options {
                    option { value: "{key}", "{text}" }
                }
            }
        }
    }
}

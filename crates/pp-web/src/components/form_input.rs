use dioxus::prelude::*;

#[component]
pub fn FormInput(
    label: String,
    value: Signal<String>,
    placeholder: Option<String>,
    input_type: Option<String>,
    error: Option<String>,
) -> Element {
    let placeholder = placeholder.unwrap_or_default();
    let input_type = input_type.unwrap_or_else(|| "text".to_string());
    let has_error = error.is_some();
    rsx! {
        div { class: "form-group",
            label { "{label}" }
            input {
                class: if has_error { "error" } else { "" },
                r#type: "{input_type}",
                placeholder: "{placeholder}",
                value: "{value.read().clone()}",
                oninput: move |e| value.set(e.value()),
            }
            if let Some(err) = error {
                span { class: "field-error", "{err}" }
            }
        }
    }
}

#[component]
pub fn FormTextarea(
    label: String,
    value: Signal<String>,
    placeholder: Option<String>,
    rows: Option<usize>,
    error: Option<String>,
) -> Element {
    let placeholder = placeholder.unwrap_or_default();
    let rows = rows.unwrap_or(4);
    let has_error = error.is_some();
    rsx! {
        div { class: "form-group",
            label { "{label}" }
            textarea {
                class: if has_error { "error" } else { "" },
                placeholder: "{placeholder}",
                rows: "{rows}",
                value: "{value.read().clone()}",
                oninput: move |e| value.set(e.value()),
            }
            if let Some(err) = error {
                span { class: "field-error", "{err}" }
            }
        }
    }
}

#[component]
pub fn FormSelect(label: String, value: Signal<String>, options: Vec<(String, String)>, error: Option<String>) -> Element {
    let has_error = error.is_some();
    rsx! {
        div { class: "form-group",
            label { "{label}" }
            select {
                class: if has_error { "error" } else { "" },
                value: "{value.read().clone()}",
                onchange: move |e| value.set(e.value()),
                for (key , text) in options {
                    option { value: "{key}", "{text}" }
                }
            }
            if let Some(err) = error {
                span { class: "field-error", "{err}" }
            }
        }
    }
}

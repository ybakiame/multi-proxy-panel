use dioxus::prelude::*;
use dioxus_i18n::t;

#[component]
pub fn ConfirmDialog(
    title: String,
    message: String,
    show: Signal<bool>,
    on_confirm: EventHandler<()>,
    confirm_text: Option<String>,
) -> Element {
    if !*show.read() {
        return rsx! {};
    }
    let confirm = confirm_text.unwrap_or_else(|| t!("common-confirm").to_string());
    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_modal: "true",
            aria_labelledby: "confirm-title",
            onclick: move |_| show.set(false),
            div {
                class: "modal",
                onclick: |e| e.stop_propagation(),
                h3 { id: "confirm-title", "{title}" }
                div { class: "modal-body", p { "{message}" } }
                div { class: "modal-actions",
                    button {
                        class: "danger",
                        onclick: move |_| {
                            show.set(false);
                            on_confirm.call(());
                        },
                        "{confirm}"
                    }
                    button {
                        onclick: move |_| show.set(false),
                        {t!("common-cancel")}
                    }
                }
            }
        }
    }
}

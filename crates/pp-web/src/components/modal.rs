use dioxus::prelude::*;
use dioxus_i18n::t;

#[component]
pub fn Modal(
    title: String,
    show: Signal<bool>,
    on_confirm: EventHandler<()>,
    confirm_text: Option<String>,
    children: Element,
) -> Element {
    if !show.read().clone() {
        return rsx! {};
    }
    let confirm_text = confirm_text.unwrap_or_else(|| t!("modal-confirm").to_string());
    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| show.set(false),
            div {
                class: "modal",
                onclick: |e| e.stop_propagation(),
                h3 { "{title}" }
                div { class: "modal-body", {children} }
                div { class: "modal-actions",
                    button {
                        onclick: move |_| {
                            on_confirm.call(());
                        },
                        "{confirm_text}"
                    }
                    button {
                        class: "danger",
                        onclick: move |_| show.set(false),
                        {t!("common-cancel")}
                    }
                }
            }
        }
    }
}

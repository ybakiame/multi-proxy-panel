use dioxus::prelude::*;
use serde_json::json;

use crate::api;

#[component]
pub fn NodeRow(
    id: String,
    name: String,
    hostname: String,
    address: String,
    status: String,
) -> Element {
    let push_id = id.clone();
    let delete_id = id;

    rsx! {
        tr {
            td { "{name}" }
            td { "{hostname}" }
            td { "{address}" }
            td { "{status}" }
            td {
                button {
                    onclick: move |_| {
                        let id = push_id.clone();
                        spawn(async move {
                            let payload = json!({ "core_type": "sing-box", "restart": true, "version": "1" });
                            let _ = api::push_config(&id, payload).await;
                        });
                    },
                    "Push Config"
                }
                button {
                    class: "danger",
                    onclick: move |_| {
                        let id = delete_id.clone();
                        spawn(async move {
                            let _ = api::delete_node(&id).await;
                        });
                    },
                    "Delete"
                }
            }
        }
    }
}

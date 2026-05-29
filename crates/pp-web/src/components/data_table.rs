use dioxus::prelude::*;

#[component]
pub fn DataTable(
    headers: Vec<String>,
    empty_message: Option<String>,
    children: Element,
) -> Element {
    let empty_message = empty_message.unwrap_or_else(|| "No data".to_string());
    rsx! {
        table { class: "data-table",
            thead {
                tr {
                    for header in headers {
                        th { "{header}" }
                    }
                }
            }
            tbody {
                {children}
            }
        }
        // Empty state handled by caller via conditional rendering if needed
    }
}

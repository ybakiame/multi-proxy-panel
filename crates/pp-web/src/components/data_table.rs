use dioxus::prelude::*;

#[component]
pub fn DataTable(headers: Vec<String>, children: Element) -> Element {
    rsx! {
        div { class: "table-wrapper",
            table { class: "data-table",
                thead {
                    tr {
                        for header in headers {
                            th { scope: "col", "{header}" }
                        }
                    }
                }
                tbody {
                    {children}
                }
            }
        }
    }
}

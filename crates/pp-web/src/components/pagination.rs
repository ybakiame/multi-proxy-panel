use dioxus::prelude::*;
use dioxus_i18n::t;

#[component]
pub fn Pagination(
    page: u64,
    per_page: u64,
    total: u64,
    on_page_change: EventHandler<u64>,
    on_per_page_change: EventHandler<u64>,
) -> Element {
    let total_pages = if total == 0 {
        1
    } else {
        total.div_ceil(per_page)
    };

    rsx! {
        div { class: "pagination",
            button {
                disabled: page <= 1,
                onclick: move |_| {
                    if page > 1 {
                        on_page_change.call(1);
                    }
                },
                {t!("pagination-first")}
            }
            button {
                disabled: page <= 1,
                onclick: move |_| {
                    if page > 1 {
                        on_page_change.call(page - 1);
                    }
                },
                {t!("pagination-prev")}
            }
            span { {t!("pagination-page", current: page, total: total_pages, count: total)} }
            button {
                disabled: page >= total_pages,
                onclick: move |_| {
                    if page < total_pages {
                        on_page_change.call(page + 1);
                    }
                },
                {t!("pagination-next")}
            }
            button {
                disabled: page >= total_pages,
                onclick: move |_| {
                    if page < total_pages {
                        on_page_change.call(total_pages);
                    }
                },
                {t!("pagination-last")}
            }
            select {
                value: "{per_page}",
                onchange: move |e| {
                    if let Ok(val) = e.value().parse::<u64>() {
                        on_per_page_change.call(val);
                    }
                },
                option { value: "10", selected: per_page == 10, "10" }
                option { value: "20", selected: per_page == 20, "20" }
                option { value: "50", selected: per_page == 50, "50" }
                option { value: "100", selected: per_page == 100, "100" }
            }
        }
    }
}

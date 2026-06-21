use dioxus::prelude::*;
use time::{Date, Month};

use super::component::DatePicker;

fn parse_iso_date(s: &str) -> Option<Date> {
    if s.is_empty() {
        return None;
    }
    let date_part = s.split('T').next()?;
    let mut parts = date_part.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    let month = Month::try_from(month).ok()?;
    Date::from_calendar_date(year, month, day).ok()
}

fn format_iso_date(date: Option<Date>) -> String {
    match date {
        Some(d) => format!(
            "{:04}-{:02}-{:02}T00:00:00+00:00",
            d.year(),
            d.month() as u8,
            d.day()
        ),
        None => String::new(),
    }
}

#[component]
pub fn FormDate(label: String, value: Signal<String>, error: Option<String>) -> Element {
    let mut date = use_signal(|| parse_iso_date(&value.read()));

    use_effect(move || {
        let s = value.read().clone();
        date.set(parse_iso_date(&s));
    });

    rsx! {
        div { class: "form-group form-date",
            label { "{label}" }
            DatePicker {
                selected_date: date(),
                on_value_change: move |d: Option<Date>| {
                    value.set(format_iso_date(d));
                },
            }
            if let Some(err) = error {
                span { class: "field-error", "{err}" }
            }
        }
    }
}

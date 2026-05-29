use dioxus::prelude::*;
use dioxus_i18n::prelude::*;
use dioxus_i18n::t;
use dioxus_i18n::unic_langid::langid;

use crate::pages::*;

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    Dashboard {},
    #[route("/nodes")]
    Nodes {},
    #[route("/protocols")]
    Protocols {},
    #[route("/bindings")]
    Bindings {},
    #[route("/clients")]
    Clients {},
    #[route("/subscriptions")]
    Subscriptions {},
    #[route("/metrics")]
    Metrics {},
    #[route("/logs")]
    Logs {},
}

#[component]
pub fn App() -> Element {
    crate::i18n::init_i18n();
    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Stylesheet { href: asset!("/assets/style.css") }
        Router::<Route> {}
    }
}

#[component]
fn LangSwitch() -> Element {
    let mut i18n = i18n();
    rsx! {
        select {
            class: "lang-switch",
            onchange: move |e| {
                match e.value().as_str() {
                    "zh-CN" => i18n.set_language(langid!("zh-CN")),
                    "en-US" => i18n.set_language(langid!("en-US")),
                    _ => {}
                }
            },
            option { value: "zh-CN", "中文" }
            option { value: "en-US", "English" }
        }
    }
}

#[component]
pub fn Layout() -> Element {
    rsx! {
        div { class: "app",
            nav { class: "sidebar",
                h2 { "ProxyPanel" }
                ul {
                    li { Link { to: Route::Dashboard {}, {t!("nav-dashboard")} } }
                    li { Link { to: Route::Nodes {}, {t!("nav-nodes")} } }
                    li { Link { to: Route::Protocols {}, {t!("nav-protocols")} } }
                    li { Link { to: Route::Bindings {}, {t!("nav-bindings")} } }
                    li { Link { to: Route::Clients {}, {t!("nav-clients")} } }
                    li { Link { to: Route::Subscriptions {}, {t!("nav-subscriptions")} } }
                    li { Link { to: Route::Metrics {}, {t!("nav-metrics")} } }
                    li { Link { to: Route::Logs {}, {t!("nav-logs")} } }
                }
                LangSwitch {}
            }
            main { class: "content",
                Outlet::<Route> {}
            }
        }
    }
}

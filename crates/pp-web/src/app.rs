use dioxus::prelude::*;

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
    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Stylesheet { href: asset!("/assets/style.css") }
        Router::<Route> {}
    }
}

#[component]
pub fn Layout() -> Element {
    rsx! {
        div { class: "app",
            nav { class: "sidebar",
                h2 { "ProxyPanel" }
                ul {
                    li { Link { to: Route::Dashboard {}, "Dashboard" } }
                    li { Link { to: Route::Nodes {}, "Nodes" } }
                    li { Link { to: Route::Protocols {}, "Protocols" } }
                    li { Link { to: Route::Bindings {}, "Bindings" } }
                    li { Link { to: Route::Clients {}, "Clients" } }
                    li { Link { to: Route::Subscriptions {}, "Subscriptions" } }
                    li { Link { to: Route::Metrics {}, "Metrics" } }
                    li { Link { to: Route::Logs {}, "Logs" } }
                }
            }
            main { class: "content",
                Outlet::<Route> {}
            }
        }
    }
}

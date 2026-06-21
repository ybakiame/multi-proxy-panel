use dioxus::prelude::*;
use dioxus_i18n::prelude::*;
use dioxus_i18n::t;
use dioxus_i18n::unic_langid::langid;

use crate::auth::{AuthProvider, Login, use_auth};
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
    #[route("/groups")]
    Groups {},
    #[route("/subscriptions")]
    Subscriptions {},
    #[route("/metrics")]
    Metrics {},
    #[route("/logs")]
    Logs {},
    #[route("/api-keys")]
    ApiKeys {},
    #[route("/webhooks")]
    Webhooks {},
    #[route("/onlines")]
    Onlines {},
    #[route("/traffic")]
    Traffic {},
}

#[component]
pub fn App() -> Element {
    crate::i18n::init_i18n();
    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Stylesheet { href: asset!("/assets/style.css") }
        document::Stylesheet { href: asset!("/assets/dx-components-theme.css") }
        AuthProvider {
            Router::<Route> {}
        }
    }
}

#[component]
fn NavItem(label: String, to: Route) -> Element {
    let current = use_route::<Route>();
    let nav = navigator();
    let active = std::mem::discriminant(&to) == std::mem::discriminant(&current);
    let class = if active { "active" } else { "" };

    rsx! {
        li {
            a {
                class: "{class}",
                href: "#",
                onclick: move |e| {
                    e.prevent_default();
                    nav.push(to.clone());
                },
                {label}
            }
        }
    }
}

#[component]
pub fn Layout() -> Element {
    let mut auth = use_auth();
    let mut sidebar_open = use_signal(|| false);

    if !auth.is_authenticated() {
        return rsx! { Login {} };
    }

    rsx! {
        div { class: "mobile-header",
            button {
                class: "mobile-menu-btn",
                onclick: move |_| {
                    let current = *sidebar_open.read();
                    sidebar_open.set(!current);
                },
                "☰"
            }
            h2 { "ProxyPanel" }
            div { class: "mobile-header-spacer" }
        }
        div { class: "app",
            nav { class: if *sidebar_open.read() { "sidebar open" } else { "sidebar" },
                h2 { "ProxyPanel" }
                ul {
                    NavItem { label: t!("nav-dashboard").to_string(), to: Route::Dashboard {} }
                    NavItem { label: t!("nav-nodes").to_string(), to: Route::Nodes {} }
                    NavItem { label: t!("nav-protocols").to_string(), to: Route::Protocols {} }
                    NavItem { label: t!("nav-bindings").to_string(), to: Route::Bindings {} }
                    NavItem { label: t!("nav-clients").to_string(), to: Route::Clients {} }
                    NavItem { label: t!("nav-groups").to_string(), to: Route::Groups {} }
                    NavItem { label: t!("nav-subscriptions").to_string(), to: Route::Subscriptions {} }
                    NavItem { label: t!("nav-metrics").to_string(), to: Route::Metrics {} }
                    NavItem { label: t!("nav-logs").to_string(), to: Route::Logs {} }
                    NavItem { label: t!("nav-api-keys").to_string(), to: Route::ApiKeys {} }
                    NavItem { label: t!("nav-webhooks").to_string(), to: Route::Webhooks {} }
                    NavItem { label: t!("nav-onlines").to_string(), to: Route::Onlines {} }
                    NavItem { label: t!("nav-traffic").to_string(), to: Route::Traffic {} }
                }
                div { class: "sidebar-footer",
                    select {
                        class: "lang-switch",
                        aria_label: t!("common-language").to_string(),
                        onchange: move |e| {
                            match e.value().as_str() {
                                "zh-CN" => i18n().set_language(langid!("zh-CN")),
                                "en-US" => i18n().set_language(langid!("en-US")),
                                _ => {}
                            }
                        },
                        option { value: "zh-CN", "中文" }
                        option { value: "en-US", "English" }
                    }
                    button {
                        class: "danger small",
                        onclick: move |_| auth.logout(),
                        {t!("common-logout")}
                    }
                }
            }
            main { class: "content",
                Outlet::<Route> {}
            }
        }
    }
}

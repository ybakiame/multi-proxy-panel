//! Authentication primitives for the web admin panel.
//!
//! The panel stores the Hub API key in `localStorage` (web) and sends it as
//! `Authorization: Bearer <key>` with every request.

use dioxus::prelude::*;

use crate::api::{clear_api_key, get_api_key, set_api_key};

/// Shared auth state. Mutating the token signal re-renders the layout and
/// switches between the login screen and the authenticated app.
#[derive(Clone, Copy)]
pub struct AuthState {
    pub token: Signal<Option<String>>,
}

impl AuthState {
    pub fn is_authenticated(&self) -> bool {
        self.token
            .read()
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    pub fn login(&mut self, key: &str) {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            set_api_key(trimmed);
            self.token.set(Some(trimmed.to_string()));
        }
    }

    pub fn logout(&mut self) {
        clear_api_key();
        self.token.set(None);
    }
}

pub fn use_auth() -> AuthState {
    use_context::<AuthState>()
}

/// Provide auth state to the rest of the app.
#[component]
pub fn AuthProvider(children: Element) -> Element {
    let token = use_signal(get_api_key);
    use_context_provider(|| AuthState { token });
    rsx! { {children} }
}

/// Simple API-key login screen.
///
/// The key is validated against the Hub before switching to the authenticated
/// app, so an invalid or truncated key does not silently render empty pages.
#[component]
pub fn Login() -> Element {
    let mut key_input = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut loading = use_signal(|| false);
    let auth = use_auth();
    let nav = navigator();

    rsx! {
        div { class: "login-page",
            div { class: "login-box",
                h2 { "ProxyPanel" }
                p { "Enter your Hub API key to continue" }
                div { class: "form-group",
                    input {
                        r#type: "password",
                        placeholder: "API key",
                        disabled: *loading.read(),
                        value: "{key_input.read().clone()}",
                        oninput: move |e| key_input.set(e.value()),
                    }
                }
                if let Some(err) = error.read().as_ref() {
                    p { class: "error-text", "{err}" }
                }
                button {
                    disabled: *loading.read(),
                    onclick: move |_| {
                        let key = key_input.read().clone();
                        if key.trim().is_empty() {
                            error.set(Some("API key is required".to_string()));
                            return;
                        }

                        loading.set(true);
                        error.set(None);
                        let mut auth = auth;
                        spawn({
                            let key = key.clone();
                            async move {
                                // Validate the key with a lightweight authenticated request.
                                match crate::api::validate_api_key(&key).await {
                                    Ok(_) => {
                                        auth.login(&key);
                                        nav.push(crate::app::Route::Dashboard {});
                                    }
                                    Err(crate::api::ApiError::Unauthorized) => {
                                        error.set(Some("Invalid API key".to_string()));
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("Failed to verify key: {}", e)));
                                    }
                                }
                                loading.set(false);
                            }
                        });
                    },
                    if *loading.read() { "Verifying..." } else { "Login" }
                }
            }
        }
    }
}

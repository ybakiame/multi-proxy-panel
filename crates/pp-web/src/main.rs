//! ProxyPanel Web — Dioxus frontend for Hub management.

use dioxus::prelude::*;

mod api;
mod app;
mod components;
mod pages;

use app::App;

fn main() {
    dioxus::logger::init(dioxus::logger::tracing::Level::INFO).expect("failed to init logger");
    launch(App);
}

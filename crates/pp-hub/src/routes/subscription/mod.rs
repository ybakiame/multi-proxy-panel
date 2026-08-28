//! Subscription route handlers.
//!
//! Re-exports all public handlers so that `crate::routes::subscription::*`
//! continues to resolve exactly as before the split.

mod access;
mod generator;
mod template;

pub use access::{serve_subscription, serve_subscription_qr};
pub use template::{
    create_subscription, create_template, delete_subscription, delete_template, list_subscriptions,
    list_templates, update_subscription, update_template,
};

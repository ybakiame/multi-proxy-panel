//! Desktop notification notifier (via [`Notifier`] trait).

use pp_script::Notifier;

/// Desktop notification notifier: `tracing::info` + `notify-rust` desktop notification
/// (when `notify-rust` feature is enabled).
pub struct TracingNotifier;

impl Default for TracingNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl TracingNotifier {
    /// Create a new notifier.
    pub fn new() -> Self {
        Self
    }
}

impl Notifier for TracingNotifier {
    fn notify(&self, title: &str, subtitle: &str, body: &str, options: Option<serde_json::Value>) {
        tracing::info!(title, subtitle, body, options = ?options, "desktop notification");
    }
}

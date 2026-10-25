//! Bounded, redacted records shared by persisted Hub and forwarded Agent logs.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use tracing::field::{Field, Visit};

pub const MAX_LOG_TEXT: usize = 8192;
const MAX_FIELDS: usize = 32;

static SECRETS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b((?:access[_-]?)?token|password|passwd|authorization|api[_-]?key|secret)\b([\s\"']*[:=]\s*)(?:\"[^\"]*\"|'[^']*'|(?:bearer\s+)?[^\s,;\}\]]+)"#)
        .ok()
});
static URL_SECRETS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r#"(https?://)(?:[^/\s:@]+:[^/\s@]+@)|(/sub/)[^\s/?#\"<>]+|([?&])[^\s\"<>]+|#[^\s\"<>]+"#,
    )
    .ok()
});

/// Remove common credential assignments and URL credentials/query/fragment tokens.
pub fn sanitize_log_text(text: &str) -> String {
    let (Some(secrets), Some(urls)) = (&*SECRETS, &*URL_SECRETS) else {
        return "[log redaction unavailable]".into();
    };
    let redacted = secrets.replace_all(text, "$1$2[REDACTED]");
    let redacted = urls.replace_all(&redacted, |caps: &regex::Captures<'_>| {
        format!(
            "{}[REDACTED]",
            caps.get(1)
                .or(caps.get(2))
                .or(caps.get(3))
                .map_or("", |m| m.as_str())
        )
    });
    let mut value = redacted.into_owned();
    if value.len() > MAX_LOG_TEXT {
        let mut end = MAX_LOG_TEXT;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        value.push_str(" [truncated]");
    }
    value
}

fn sensitive(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "token",
        "password",
        "passwd",
        "secret",
        "authorization",
        "api_key",
        "apikey",
        "credential",
    ]
    .iter()
    .any(|part| name.contains(part))
}

/// Redact credential fields and bound metadata independently of messages.
pub fn sanitize_log_field(name: &str, value: &str) -> String {
    if sensitive(name) {
        return "[REDACTED]".into();
    }
    let mut value = sanitize_log_text(value);
    let mut end = value.len().min(512);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value
}

/// Sanitize legacy JSON metadata before exposing it through log APIs.
pub fn sanitize_log_metadata(value: serde_json::Value) -> serde_json::Value {
    fn clean(value: serde_json::Value, depth: usize) -> serde_json::Value {
        use serde_json::Value;
        if depth > 4 {
            return Value::String("[metadata truncated]".into());
        }
        match value {
            Value::String(value) => Value::String(sanitize_log_field("value", &value)),
            Value::Object(values) => Value::Object(
                values
                    .into_iter()
                    .take(MAX_FIELDS)
                    .map(|(key, value)| {
                        let value = if sensitive(&key) {
                            Value::String("[REDACTED]".into())
                        } else {
                            clean(value, depth + 1)
                        };
                        (key, value)
                    })
                    .collect(),
            ),
            Value::Array(values) => Value::Array(
                values
                    .into_iter()
                    .take(MAX_FIELDS)
                    .map(|v| clean(v, depth + 1))
                    .collect(),
            ),
            value => value,
        }
    }
    clean(value, 0)
}

/// Capture safe tracing fields without persisting arbitrary Debug payloads.
#[derive(Default)]
pub struct LogVisitor {
    pub message: String,
    pub fields: HashMap<String, String>,
}

impl LogVisitor {
    fn store(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = sanitize_log_text(value);
        } else if self.fields.len() < MAX_FIELDS {
            self.fields
                .insert(field.name().into(), sanitize_log_field(field.name(), value));
        }
    }
}

impl Visit for LogVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.store(field, value);
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.store(field, &value.to_string());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.store(field, &value.to_string());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.store(field, &value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if sensitive(field.name()) && field.name() != "message" {
            self.store(field, "[REDACTED]");
        } else if field.name() == "message" {
            self.store(field, &format!("{value:?}"));
        } else {
            self.store(field, "[debug value omitted]");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_credentials_and_subscription_urls() {
        let text = sanitize_log_text(
            "password=secret token:abc Authorization: Bearer xyz api_key=foo https://user:pass@example.com/sub/subtoken?token=bar#fragment",
        );
        for secret in [
            "secret",
            "abc",
            "xyz",
            "foo",
            "user:pass",
            "subtoken",
            "bar",
            "fragment",
        ] {
            assert!(!text.contains(secret), "credential leaked: {text}");
        }
        assert!(text.contains("example.com"));
    }

    #[test]
    fn sanitizes_nested_legacy_metadata_and_bounds_fields() {
        let value = sanitize_log_metadata(
            serde_json::json!({"password": "private", "nested": {"api_key": "hidden", "count": 2}}),
        );
        assert_eq!(value["password"], "[REDACTED]");
        assert_eq!(value["nested"]["api_key"], "[REDACTED]");
        assert_eq!(value["nested"]["count"], 2);
        assert!(sanitize_log_field("error", &"错误".repeat(5000)).len() <= 512);
    }

    #[test]
    fn preserves_readable_events_and_bounds_unicode() {
        assert_eq!(
            sanitize_log_text("agent connected node=123"),
            "agent connected node=123"
        );
        let value = sanitize_log_text(&"错误".repeat(5000));
        assert!(value.len() <= MAX_LOG_TEXT + 12);
        assert!(value.ends_with(" [truncated]"));
    }
}

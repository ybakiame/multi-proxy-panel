//! Helpers for reading normalized protocol settings.
//!
//! Protocol configs store values in slightly different shapes depending on
//! whether they were created from the web form (arrays for multi-value fields)
//! or from raw JSON. Client credentials are injected as a `clients` array,
//! which should take precedence over placeholder values stored in the config.

use serde_json::Value;

/// Get the first non-empty server name from `server_names` / `reality_server_names`.
/// Accepts both a comma-separated string and a JSON array.
pub fn first_server_name(settings: &Value) -> Option<String> {
    let raw = settings
        .get("server_names")
        .or_else(|| settings.get("reality_server_names"))?;

    let names: Vec<String> = if let Some(s) = raw.as_str() {
        s.split(',')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect()
    } else if let Some(arr) = raw.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|v| !v.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    names.into_iter().next()
}

/// Get the first non-empty short id from `short_id` / `reality_short_id`.
/// Accepts both a comma-separated string and a JSON array.
pub fn first_short_id(settings: &Value) -> Option<String> {
    let raw = settings
        .get("short_id")
        .or_else(|| settings.get("reality_short_id"))?;

    let ids: Vec<String> = if let Some(s) = raw.as_str() {
        s.split(',')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect()
    } else if let Some(arr) = raw.as_array() {
        arr.iter()
            .filter_map(|v| {
                v.as_str()
                    .map(|s| s.trim().to_string())
                    .or_else(|| v.as_u64().map(|n| format!("{:x}", n)))
            })
            .filter(|v| !v.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    ids.into_iter().next()
}

/// Get the effective client UUID for VLESS/VMess/TUIC.
/// Prefers the injected `clients` array over top-level placeholder `id`/`uuid`.
pub fn client_uuid(settings: &Value) -> Option<String> {
    if let Some(arr) = settings.get("clients").and_then(|v| v.as_array()) {
        if let Some(uuid) = arr
            .first()
            .and_then(|c| c.get("id").or_else(|| c.get("uuid")))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(uuid.to_string());
        }
    }

    settings
        .get("id")
        .or_else(|| settings.get("uuid"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Get the effective client password for hysteria2/trojan/anytls/shadowsocks.
/// Prefers the injected `clients` array over top-level placeholder `password`.
pub fn client_password(settings: &Value) -> Option<String> {
    if let Some(arr) = settings.get("clients").and_then(|v| v.as_array()) {
        if let Some(password) = arr
            .first()
            .and_then(|c| c.get("password"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(password.to_string());
        }
    }

    settings
        .get("password")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn first_server_name_handles_string_and_array() {
        assert_eq!(
            first_server_name(&json!({"server_names": "example.com, www.example.com"})).as_deref(),
            Some("example.com")
        );
        assert_eq!(
            first_server_name(&json!({"server_names": ["example.com", "www.example.com"]}))
                .as_deref(),
            Some("example.com")
        );
        assert_eq!(first_server_name(&json!({"server_names": ""})), None);
        assert_eq!(first_server_name(&json!({})), None);
    }

    #[test]
    fn first_short_id_handles_string_and_array() {
        assert_eq!(
            first_short_id(&json!({"short_id": "0123456789abcdef, abcdef"})).as_deref(),
            Some("0123456789abcdef")
        );
        assert_eq!(
            first_short_id(&json!({"short_id": ["0123456789abcdef", "abcdef"]})).as_deref(),
            Some("0123456789abcdef")
        );
        assert_eq!(first_short_id(&json!({"short_id": ""})), None);
    }

    #[test]
    fn client_uuid_prefers_clients_array() {
        let settings = json!({
            "id": "",
            "clients": [{"id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4"}]
        });
        assert_eq!(
            client_uuid(&settings).as_deref(),
            Some("a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4")
        );

        let placeholder = json!({"id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"});
        assert_eq!(
            client_uuid(&placeholder).as_deref(),
            Some("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
        );
    }

    #[test]
    fn client_password_prefers_clients_array() {
        let settings = json!({
            "password": "",
            "clients": [{"password": "secret"}]
        });
        assert_eq!(client_password(&settings).as_deref(), Some("secret"));

        let placeholder = json!({"password": "placeholder"});
        assert_eq!(
            client_password(&placeholder).as_deref(),
            Some("placeholder")
        );
    }
}

//! Pure validation and conversion utilities shared between the Tauri command layer
//! and any other consumers.

use pp_script::ScriptDialect;

/// Validate YAML override: non-empty content must be a parseable YAML mapping.
///
/// Returns `Ok(())` for empty or null input; returns an error with a message
/// if the YAML is not a mapping (object).
pub fn validate_yaml_override(yaml: &str) -> Result<(), String> {
    if yaml.trim().is_empty() {
        return Ok(());
    }
    let patch: serde_json::Value =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML override parse error: {e}"))?;
    if patch.is_null() {
        return Ok(());
    }
    if !patch.is_object() {
        return Err("YAML override must be a mapping (object)".to_string());
    }
    Ok(())
}

/// Rough check that a JS override defines a `main` function.
///
/// Returns `Ok(())` for empty input; returns an error if neither
/// `function main` nor `main(` is found.
pub fn validate_js_override(js: &str) -> Result<(), String> {
    if js.trim().is_empty() {
        return Ok(());
    }
    if !js.contains("function main") && !js.contains("main(") {
        return Err(
            "JS override must define a main function (function main(config) { ... return config; })"
                .to_string(),
        );
    }
    Ok(())
}

/// Validate a remote override URL: non-empty must start with `http://` or `https://`.
///
/// Returns `Ok(())` for `None`, empty string, or a valid HTTP(S) URL.
pub fn validate_remote_url(url: &Option<String>) -> Result<(), String> {
    if let Some(url) = url {
        let url = url.trim();
        if !(url.is_empty() || url.starts_with("http://") || url.starts_with("https://")) {
            return Err("Remote override URL must start with http:// or https://".to_string());
        }
    }
    Ok(())
}

/// Normalize an optional URL: trim whitespace and convert empty string to `None`.
pub fn normalize_optional_url(url: Option<String>) -> Option<String> {
    url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty())
}

/// Validate that a subscription URL starts with `http://` or `https://`.
pub fn validate_subscription_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err("Subscription URL must start with http:// or https://".to_string())
    }
}

/// Serialize `RemoteKind` to string (matches `RemoteResourceView.kind` serde).
pub fn remote_kind_str(kind: crate::RemoteKind) -> &'static str {
    match kind {
        crate::RemoteKind::Script => "Script",
        crate::RemoteKind::Snippet => "Snippet",
    }
}

/// String representation of `ScriptDialect` (matches frontend serde).
///
/// QX is merged into the Loon ecosystem; detected QuantumultX is mapped to `Loon`.
pub fn script_dialect_str(dialect: ScriptDialect) -> &'static str {
    match dialect {
        ScriptDialect::QuantumultX => "Loon",
        ScriptDialect::Surge => "Surge",
        ScriptDialect::Loon => "Loon",
    }
}

/// String representation of `SubFormat`.
pub fn sub_format_str(format: crate::SubFormat) -> &'static str {
    match format {
        crate::SubFormat::ShareLinks => "ShareLinks",
        crate::SubFormat::ClashYaml => "ClashYaml",
        crate::SubFormat::SingBoxJson => "SingBoxJson",
    }
}

/// Subscription format compatibility check.
///
/// The client only supports the sing-box core; all sniffed subscription formats
/// (share links / clash YAML / sing-box JSON) are converted to sing-box nodes at
/// fetch time, so this always succeeds. Kept as the preview gate entry for the
/// command layer.
pub fn check_preview_core_compat(_format: crate::SubFormat) -> Result<(), String> {
    Ok(())
}

/// Persist rule mode after validation.
///
/// Validates the mode is one of `rule` / `global` / `direct`, then writes it
/// to `client.json`.
pub fn set_rule_mode_persist(data_dir: &std::path::Path, mode: &str) -> Result<(), String> {
    match mode {
        "rule" | "global" | "direct" => {}
        _ => return Err("Invalid rule mode".to_string()),
    }
    let mut config = crate::config::ClientConfig::load(data_dir)
        .map_err(|e| format!("Failed to load config: {e}"))?;
    config.rule_mode = mode.to_string();
    config
        .save()
        .map_err(|e| format!("Failed to save config: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_yaml_override_accepts_empty_and_null() {
        assert!(validate_yaml_override("").is_ok());
        assert!(validate_yaml_override("   ").is_ok());
        assert!(validate_yaml_override("null").is_ok());
    }

    #[test]
    fn validate_yaml_override_rejects_non_object() {
        assert!(validate_yaml_override("- a\n- b").is_err());
        assert!(validate_yaml_override("hello").is_err());
    }

    #[test]
    fn validate_yaml_override_accepts_mapping() {
        assert!(validate_yaml_override("dns:\n  enabled: true").is_ok());
    }

    #[test]
    fn validate_js_override_accepts_empty() {
        assert!(validate_js_override("").is_ok());
        assert!(validate_js_override("   ").is_ok());
    }

    #[test]
    fn validate_js_override_rejects_missing_main() {
        assert!(validate_js_override("console.log(1)").is_err());
    }

    #[test]
    fn validate_js_override_accepts_function_main() {
        assert!(validate_js_override("function main(c) { return c; }").is_ok());
        assert!(validate_js_override("main(config)").is_ok());
    }

    #[test]
    fn validate_remote_url_accepts_none_empty_and_http() {
        assert!(validate_remote_url(&None).is_ok());
        assert!(validate_remote_url(&Some(String::new())).is_ok());
        assert!(validate_remote_url(&Some("  ".to_string())).is_ok());
        assert!(validate_remote_url(&Some("https://example.com".to_string())).is_ok());
    }

    #[test]
    fn validate_remote_url_rejects_invalid() {
        assert!(validate_remote_url(&Some("ftp://example.com".to_string())).is_err());
    }

    #[test]
    fn normalize_optional_url_trims_and_filters_empty() {
        assert_eq!(normalize_optional_url(None), None);
        assert_eq!(normalize_optional_url(Some(String::new())), None);
        assert_eq!(
            normalize_optional_url(Some("  https://x.com  ".to_string())),
            Some("https://x.com".to_string())
        );
    }

    #[test]
    fn validate_subscription_url_checks_scheme() {
        assert!(validate_subscription_url("https://x.com").is_ok());
        assert!(validate_subscription_url("http://x.com").is_ok());
        assert!(validate_subscription_url("ftp://x.com").is_err());
    }

    #[test]
    fn script_dialect_str_maps_qx_to_loon() {
        assert_eq!(script_dialect_str(ScriptDialect::QuantumultX), "Loon");
        assert_eq!(script_dialect_str(ScriptDialect::Surge), "Surge");
        assert_eq!(script_dialect_str(ScriptDialect::Loon), "Loon");
    }

    #[test]
    fn sub_format_str_values() {
        assert_eq!(sub_format_str(crate::SubFormat::ShareLinks), "ShareLinks");
        assert_eq!(sub_format_str(crate::SubFormat::ClashYaml), "ClashYaml");
        assert_eq!(sub_format_str(crate::SubFormat::SingBoxJson), "SingBoxJson");
    }

    /// 预览门放行：客户端仅支持 sing-box，三种订阅格式（含 ClashYaml）全部兼容。
    #[test]
    fn check_preview_core_compat_allows_all_formats() {
        assert!(check_preview_core_compat(crate::SubFormat::ShareLinks).is_ok());
        assert!(check_preview_core_compat(crate::SubFormat::SingBoxJson).is_ok());
        assert!(check_preview_core_compat(crate::SubFormat::ClashYaml).is_ok());
    }

    #[test]
    fn set_rule_mode_persist_rejects_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = crate::config::ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            std::path::PathBuf::new(),
        );
        cfg.save().unwrap();

        for invalid in ["", "bogus", "Rule", "global "] {
            let err = set_rule_mode_persist(dir.path(), invalid).unwrap_err();
            assert!(err.contains("Invalid rule mode"), "{invalid:?}: {err}");
        }
    }

    #[test]
    fn set_rule_mode_persist_persists_valid() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = crate::config::ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            std::path::PathBuf::new(),
        );
        cfg.save().unwrap();

        for mode in ["global", "direct", "rule"] {
            set_rule_mode_persist(dir.path(), mode).unwrap();
            let saved = crate::config::ClientConfig::load(dir.path()).unwrap();
            assert_eq!(
                saved.rule_mode, mode,
                "{mode} should persist to client.json"
            );
        }
    }
}

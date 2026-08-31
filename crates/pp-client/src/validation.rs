//! Pure validation and conversion utilities shared between the Tauri command layer
//! and any other consumers.

use pp_common::CoreType;
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

/// Serialize `CoreType` to frontend lowercase convention (`singbox` / `mihomo`).
pub fn core_type_str(core_type: CoreType) -> String {
    serde_json::to_value(core_type)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Parse frontend lowercase core type string (`singbox` / `mihomo`).
pub fn core_type_from_str(s: &str) -> Result<CoreType, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("invalid core type '{s}' (expected: singbox / mihomo)"))
}

/// String representation of `RemoteKind` (matches `RemoteResourceView.kind` serde).
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

/// User-visible display name of core type (`sing-box` / `mihomo`).
pub fn core_type_display_name(core_type: CoreType) -> &'static str {
    match core_type {
        CoreType::SingBox => "sing-box",
        CoreType::Mihomo => "mihomo",
    }
}

/// Subscription format <-> core type compatibility check.
///
/// Returns `Ok(())` when compatible; otherwise returns an error describing
/// the mismatch.
pub fn check_preview_core_compat(
    format: crate::SubFormat,
    core_type: CoreType,
) -> Result<(), String> {
    let compatible = match format {
        crate::SubFormat::ShareLinks => true,
        crate::SubFormat::SingBoxJson => core_type == CoreType::SingBox,
        crate::SubFormat::ClashYaml => core_type == CoreType::Mihomo,
    };
    if compatible {
        return Ok(());
    }
    let (format_name, supported_core) = if format == crate::SubFormat::ClashYaml {
        ("clash", "mihomo")
    } else {
        ("sing-box", "sing-box")
    };
    Err(format!(
        "Subscription format is {format_name}, only {supported_core} core is supported; current core type is {core_type}, please switch core type in settings"
    ))
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
    use pp_common::CoreType;

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
    fn core_type_str_roundtrip() {
        assert_eq!(core_type_str(CoreType::SingBox), "singbox");
        assert_eq!(core_type_str(CoreType::Mihomo), "mihomo");
    }

    #[test]
    fn core_type_from_str_parses_valid() {
        assert_eq!(core_type_from_str("singbox").unwrap(), CoreType::SingBox);
        assert_eq!(core_type_from_str("mihomo").unwrap(), CoreType::Mihomo);
    }

    #[test]
    fn core_type_from_str_rejects_invalid() {
        assert!(core_type_from_str("invalid").is_err());
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

    #[test]
    fn check_preview_core_compat_share_links_always_ok() {
        assert!(check_preview_core_compat(crate::SubFormat::ShareLinks, CoreType::SingBox).is_ok());
        assert!(check_preview_core_compat(crate::SubFormat::ShareLinks, CoreType::Mihomo).is_ok());
    }

    #[test]
    fn check_preview_core_compat_mismatches_error() {
        assert!(check_preview_core_compat(crate::SubFormat::ClashYaml, CoreType::SingBox).is_err());
        assert!(
            check_preview_core_compat(crate::SubFormat::SingBoxJson, CoreType::Mihomo).is_err()
        );
    }

    #[test]
    fn check_preview_core_compat_matching_ok() {
        assert!(check_preview_core_compat(crate::SubFormat::ClashYaml, CoreType::Mihomo).is_ok());
        assert!(
            check_preview_core_compat(crate::SubFormat::SingBoxJson, CoreType::SingBox).is_ok()
        );
    }

    #[test]
    fn set_rule_mode_persist_rejects_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = crate::config::ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
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
            CoreType::SingBox,
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

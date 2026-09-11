//! Experimental slice tests (schema / validation / render + merge).
//!
//! Kept in a dedicated file (one per slice domain) so the source modules stay
//! within the business-file size gate (`.agents/rules/code-organization.md`).

use crate::config_slices::*;
use serde_json::json;

fn slices_with_experimental(experimental: ExperimentalSlice) -> ConfigSlices {
    ConfigSlices {
        experimental,
        ..Default::default()
    }
}

fn cache_file(enabled: bool) -> CacheFileSlice {
    CacheFileSlice {
        enabled,
        ..Default::default()
    }
}

#[test]
fn experimental_default_is_disabled() {
    let exp = ExperimentalSlice::default();
    assert!(!exp.enabled);
    assert!(!exp.cache_file.enabled);
    assert!(exp.cache_file.path.is_empty());
    assert!(exp.cache_file.cache_id.is_empty());
    assert!(!exp.cache_file.store_fakeip);
}

#[test]
fn experimental_missing_fields_fall_back_to_defaults() {
    let exp: ExperimentalSlice =
        serde_json::from_str(r#"{"enabled":true,"cache_file":{"path":"/data/cache.db"}}"#).unwrap();
    assert!(exp.enabled);
    assert!(!exp.cache_file.enabled);
    assert_eq!(exp.cache_file.path, "/data/cache.db");
    assert!(exp.cache_file.cache_id.is_empty());
    assert!(!exp.cache_file.store_fakeip);

    let empty: ExperimentalSlice = serde_json::from_str("{}").unwrap();
    assert_eq!(empty, ExperimentalSlice::default());
}

#[test]
fn config_slices_roundtrip_includes_experimental() {
    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: true,
        cache_file: CacheFileSlice {
            enabled: true,
            path: "/data/cache.db".to_string(),
            cache_id: "main".to_string(),
            store_fakeip: true,
        },
    });
    let text = serde_json::to_string(&slices).unwrap();
    let back: ConfigSlices = serde_json::from_str(&text).unwrap();
    assert_eq!(slices, back);
}

#[test]
fn validate_accepts_enabled_slice_with_path() {
    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: true,
        cache_file: CacheFileSlice {
            enabled: true,
            path: "/data/cache.db".to_string(),
            ..Default::default()
        },
    });
    slices.validate().unwrap();
}

#[test]
fn validate_accepts_empty_path() {
    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: true,
        cache_file: cache_file(true),
    });
    slices.validate().unwrap();
}

#[test]
fn validate_rejects_blank_path() {
    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: true,
        cache_file: CacheFileSlice {
            enabled: true,
            path: "   ".to_string(),
            ..Default::default()
        },
    });
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("must not be blank"), "{err}");
}

#[test]
fn validate_skips_disabled_slice() {
    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: false,
        cache_file: CacheFileSlice {
            enabled: true,
            path: "   ".to_string(),
            ..Default::default()
        },
    });
    slices.validate().unwrap();
}

#[test]
fn render_cache_file_emits_only_set_fields() {
    let value = render_cache_file(&CacheFileSlice {
        enabled: true,
        path: "/data/cache.db".to_string(),
        cache_id: "main".to_string(),
        store_fakeip: true,
    });
    assert_eq!(
        value,
        json!({
            "enabled": true,
            "path": "/data/cache.db",
            "cache_id": "main",
            "store_fakeip": true
        })
    );

    let minimal = render_cache_file(&cache_file(true));
    assert_eq!(minimal, json!({ "enabled": true }));
}

#[test]
fn render_cache_file_disabled_writes_enabled_false() {
    let value = render_cache_file(&CacheFileSlice {
        enabled: false,
        path: "/ignored.db".to_string(),
        cache_id: "ignored".to_string(),
        store_fakeip: true,
    });
    assert_eq!(value, json!({ "enabled": false }));
}

#[test]
fn apply_merges_cache_file_preserving_clash_api() {
    let mut config = json!({
        "experimental": {
            "clash_api": { "external_controller": "127.0.0.1:9090" }
        }
    });
    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: true,
        cache_file: CacheFileSlice {
            enabled: true,
            path: "/data/cache.db".to_string(),
            ..Default::default()
        },
    });

    apply_config_slices(&mut config, &slices).unwrap();
    assert_eq!(
        config["experimental"]["clash_api"]["external_controller"],
        "127.0.0.1:9090"
    );
    assert_eq!(config["experimental"]["cache_file"]["enabled"], true);
    assert_eq!(
        config["experimental"]["cache_file"]["path"],
        "/data/cache.db"
    );
}

#[test]
fn apply_creates_experimental_object_when_absent() {
    let mut config = json!({ "outbounds": [] });
    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: true,
        cache_file: cache_file(true),
    });

    apply_config_slices(&mut config, &slices).unwrap();
    assert_eq!(
        config["experimental"]["cache_file"],
        json!({ "enabled": true })
    );
}

#[test]
fn apply_disabled_slice_leaves_config_unchanged() {
    let mut config = json!({
        "experimental": { "clash_api": { "external_controller": "127.0.0.1:9090" } }
    });
    let original = config.clone();

    let slices = slices_with_experimental(ExperimentalSlice {
        enabled: false,
        cache_file: CacheFileSlice {
            enabled: true,
            path: "/data/cache.db".to_string(),
            ..Default::default()
        },
    });
    apply_config_slices(&mut config, &slices).unwrap();
    assert_eq!(config, original);
}

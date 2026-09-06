//! Rule set download / cache / custom rule set tests (split out of `ruleset.rs`
//! to stay within the business-file size gate; see `.agents/rules/code-organization.md`).

use super::*;
use crate::local_override::store::built_in_rule_set_subscriptions;

#[test]
fn cache_file_path_formats_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let p1 = mgr.cache_file_path("geoip-cn");
    assert_eq!(p1.file_name().unwrap(), "geoip-cn.srs");
}

#[test]
fn build_rule_set_refs_skips_unsubscribed() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let ovr = LocalOverride {
        rule_set_subscriptions: built_in_rule_set_subscriptions(),
        ..Default::default()
    };
    // None subscribed, none cached.
    let refs = mgr.build_rule_set_refs(&ovr);
    assert!(refs.is_empty());
}

#[test]
fn build_rule_set_refs_includes_cached_subscribed() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let mut subs = built_in_rule_set_subscriptions();
    subs[0].subscribed = true;
    let ovr = LocalOverride {
        rule_set_subscriptions: subs,
        ..Default::default()
    };
    // Create fake cache.
    std::fs::create_dir_all(mgr.cache_dir()).unwrap();
    std::fs::write(mgr.cache_file_path("geoip-cn"), "fake").unwrap();

    let refs = mgr.build_rule_set_refs(&ovr);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].tag, "geoip-cn");
    assert!(matches!(refs[0].kind, RuleSetKind::SingBoxRemote));
}

// -----------------------------------------------------------------------
// Custom rule sets
// -----------------------------------------------------------------------

fn custom(id: &str, tag: &str, source: CustomRuleSetSource, enabled: bool) -> CustomRuleSet {
    CustomRuleSet {
        id: id.to_string(),
        name: String::new(),
        tag: tag.to_string(),
        source,
        enabled,
        last_updated: 0,
    }
}

#[test]
fn custom_rule_set_file_path_matches_format() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let dir_path = dir.path().join("rulesets").join("custom");
    assert_eq!(mgr.custom_rule_set_dir(), dir_path);
    assert_eq!(
        mgr.custom_rule_set_file_path("abc", RuleSetFormat::Binary)
            .file_name()
            .unwrap(),
        "abc.srs"
    );
    assert_eq!(
        mgr.custom_rule_set_file_path("abc", RuleSetFormat::Source)
            .file_name()
            .unwrap(),
        "abc.json"
    );
}

#[test]
fn sync_custom_rule_set_files_writes_manual_and_cleans_orphans() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());

    // Stray files that must be cleaned after sync: an orphaned remote
    // cache (.srs), an orphaned manual file (.json), and a stale file of a
    // rule set that switched source kind.
    std::fs::create_dir_all(mgr.custom_rule_set_dir()).unwrap();
    for name in ["gone.srs", "gone.json", "switched.srs", "keep.srs"] {
        std::fs::write(mgr.custom_rule_set_dir().join(name), "stale").unwrap();
    }

    // New list: manual "keep" (same id but switched from Remote binary to
    // Manual source) plus a fresh manual "manual-a".
    let manual_a = custom(
        "manual-a",
        "manual-a-tag",
        CustomRuleSetSource::Manual {
            content: "[{\"rules\":[]}]".to_string(),
        },
        true,
    );
    let switched = custom(
        "switched",
        "switched-tag",
        CustomRuleSetSource::Manual {
            content: "manual-content".to_string(),
        },
        true,
    );
    mgr.sync_custom_rule_set_files(&[manual_a.clone(), switched.clone()])
        .unwrap();

    // Manual contents persisted.
    let manual_a_file = mgr.custom_rule_set_file_path("manual-a", RuleSetFormat::Source);
    assert_eq!(
        std::fs::read_to_string(&manual_a_file).unwrap(),
        "[{\"rules\":[]}]"
    );
    let switched_file = mgr.custom_rule_set_file_path("switched", RuleSetFormat::Source);
    assert_eq!(
        std::fs::read_to_string(&switched_file).unwrap(),
        "manual-content"
    );

    // Orphaned files removed; retained files kept.
    let remaining: Vec<String> = std::fs::read_dir(mgr.custom_rule_set_dir())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(!remaining.contains(&"gone.srs".to_string()));
    assert!(!remaining.contains(&"gone.json".to_string()));
    assert!(!remaining.contains(&"switched.srs".to_string()));
    assert!(remaining.contains(&"switched.json".to_string()));
    assert!(remaining.contains(&"manual-a.json".to_string()));

    // Removing a rule set from the list cleans its backing file on next sync.
    mgr.sync_custom_rule_set_files(&[switched]).unwrap();
    assert!(!manual_a_file.exists());
    assert!(switched_file.exists());
}

#[tokio::test]
async fn update_all_subscribed_covers_enabled_custom_remote() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route("/bin", axum::routing::get(|| async { "binary-body" }))
        .route(
            "/missing",
            axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());

    let mut ovr = LocalOverride::default();
    // Enabled remote → downloaded.
    ovr.custom_rule_sets.push(custom(
        "c-ok",
        "ok-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/bin"),
            format: RuleSetFormat::Binary,
        },
        true,
    ));
    // Enabled remote but failing URL → best-effort skip (count unaffected).
    ovr.custom_rule_sets.push(custom(
        "c-fail",
        "fail-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/missing"),
            format: RuleSetFormat::Source,
        },
        true,
    ));
    // Disabled remote → skipped.
    ovr.custom_rule_sets.push(custom(
        "c-disabled",
        "disabled-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/bin"),
            format: RuleSetFormat::Source,
        },
        false,
    ));
    // Manual → no download.
    ovr.custom_rule_sets.push(custom(
        "c-manual",
        "manual-tag",
        CustomRuleSetSource::Manual {
            content: "[]".to_string(),
        },
        true,
    ));

    let updated = mgr.update_all_subscribed(&mut ovr).await.unwrap();
    assert_eq!(
        updated, 1,
        "only the reachable enabled remote should update"
    );

    let bin = mgr.custom_rule_set_file_path("c-ok", RuleSetFormat::Binary);
    assert_eq!(std::fs::read_to_string(&bin).unwrap(), "binary-body");
    assert!(ovr.custom_rule_sets[0].last_updated > 0);

    // Failed/disabled/manual never wrote files nor bumped timestamps.
    assert!(
        !mgr.custom_rule_set_file_path("c-fail", RuleSetFormat::Source)
            .exists()
    );
    assert!(
        !mgr.custom_rule_set_file_path("c-disabled", RuleSetFormat::Source)
            .exists()
    );
    assert!(
        !mgr.custom_rule_set_file_path("c-manual", RuleSetFormat::Source)
            .exists()
    );
    assert_eq!(ovr.custom_rule_sets[1].last_updated, 0);
    assert_eq!(ovr.custom_rule_sets[2].last_updated, 0);
    assert_eq!(ovr.custom_rule_sets[3].last_updated, 0);
}

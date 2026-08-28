use super::*;
use pp_script::ScriptDialect;

#[test]
fn save_load_roundtrip_applies_defaults_and_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());

    // File does not exist -> empty list
    assert!(manager.load().unwrap().is_empty());

    let remotes = vec![RemoteResource {
        name: "rules".into(),
        url: "http://example.com/rules.conf".into(),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::QuantumultX,
        ..RemoteResource::default()
    }];
    manager.save(&remotes).unwrap();
    assert!(manager.remotes_file().exists());
    assert_eq!(manager.load().unwrap(), remotes);
}

#[tokio::test]
async fn fetch_script_downloads_js_to_scripts_dir() {
    let base = spawn_remote_server(|_| String::new()).await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "my-script".into(),
        url: format!("{base}/script.js"),
        kind: RemoteKind::Script,
        ..RemoteResource::default()
    }];

    let report = manager.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1);
    assert_eq!(report.scripts, 1);
    assert!(
        report.warnings.is_empty(),
        "warnings: {:?}",
        report.warnings
    );

    let path = dir.path().join("scripts/my-script.js");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "const script = 3;");
}

#[tokio::test]
async fn fetch_snippet_aggregates_and_recompiles_cached_rules() {
    let base = spawn_remote_server(|base| {
        format!(
            "[rewrite_local]\n\
                 ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
                 ^https?://example\\.com/rsp script-response-body {base}/hook.js\n\
                 \n\
                 [task_local]\n\
                 0 9 * * * {base}/task.js, tag=daily-checkin\n\
                 \n\
                 [mitm]\n\
                 hostname = *.example.com, api.example2.com\n"
        )
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "rules".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::QuantumultX,
        ..RemoteResource::default()
    }];

    let report = manager.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1);
    assert_eq!(report.rewrites, 1);
    assert_eq!(report.scripts, 1);
    assert_eq!(report.tasks, 1);
    assert!(
        report.warnings.is_empty(),
        "warnings: {:?}",
        report.warnings
    );

    // cache file generated
    assert!(dir.path().join("remote_cache/rules.json").exists());

    // load_cached: recompile Regex, backfill source, deduplicate hostname
    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.rewrites.len(), 1);
    assert_eq!(
        merged.rewrites[0].pattern.as_str(),
        r"^https?://example\.com/api/(.*)"
    );
    match &merged.rewrites[0].kind {
        pp_mitm::RewriteKind::UrlRewrite { target } => {
            assert_eq!(target, "https://cdn.example.com/api/$1");
        }
        other => panic!("unexpected kind: {other:?}"),
    }
    assert_eq!(merged.scripts.len(), 1);
    assert_eq!(merged.scripts[0].name, "hook-0");
    assert_eq!(merged.scripts[0].source, "const hook = 1;");
    assert_eq!(merged.task_scripts.len(), 1);
    assert_eq!(merged.task_scripts[0].name, "daily-checkin");
    assert_eq!(merged.task_scripts[0].source, "const task = 2;");
    assert_eq!(
        merged.hostnames,
        vec!["*.example.com".to_string(), "api.example2.com".to_string()]
    );
}

#[tokio::test]
async fn partial_url_failure_records_warning_without_blocking_others() {
    let base = spawn_remote_server(|base| {
        format!(
            "[rewrite_local]\n\
                 ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
                 ^https?://example\\.com/rsp script-response-body {base}/missing.js\n"
        )
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![
        RemoteResource {
            name: "bad".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::QuantumultX,
            ..RemoteResource::default()
        },
        RemoteResource {
            name: "good".into(),
            url: format!("{base}/script.js"),
            kind: RemoteKind::Script,
            ..RemoteResource::default()
        },
    ];

    let report = manager.fetch_all(&remotes).await;
    // Both remotes fetched successfully; bad's hook script 404 skipped
    assert_eq!(report.fetched, 2);
    assert_eq!(report.scripts, 1); // only good script
    assert_eq!(report.rewrites, 1); // bad's rewrite still cached
    assert!(
        report.warnings.iter().any(|w| w.contains("missing.js")),
        "warnings: {:?}",
        report.warnings
    );

    // good written to disk, bad snippet cache still generated (rewrite kept, scripts skipped)
    assert!(dir.path().join("scripts/good.js").exists());
    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.rewrites.len(), 1);
    assert!(merged.scripts.is_empty());
    assert!(merged.task_scripts.is_empty());
}

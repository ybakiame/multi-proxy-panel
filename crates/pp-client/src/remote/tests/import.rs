use super::*;
use axum::http::StatusCode;
use pp_script::ScriptDialect;

/// Start local import test server: provides CamScanner script and 404 endpoint.
async fn spawn_import_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/camscanner.js",
            axum::routing::get(|| async { "const camscanner = 1;" }),
        )
        .route(
            "/missing.js",
            axum::routing::get(|| async { (StatusCode::NOT_FOUND, "not found") }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[test]
fn detect_resource_from_url_maps_suffixes_to_kind_and_dialect() {
    assert_eq!(
        detect_resource_from_url("https://example.com/config.sgmodule"),
        Some((RemoteKind::Snippet, ScriptDialect::Surge))
    );
    assert_eq!(
        detect_resource_from_url("https://example.com/conf.plugin"),
        Some((RemoteKind::Snippet, ScriptDialect::Loon))
    );
    assert_eq!(
        detect_resource_from_url("https://example.com/rules.loon"),
        Some((RemoteKind::Snippet, ScriptDialect::Loon))
    );
    assert_eq!(
        detect_resource_from_url("https://example.com/rules.conf"),
        Some((RemoteKind::Snippet, ScriptDialect::QuantumultX))
    );
    assert_eq!(
        detect_resource_from_url("https://example.com/script.js"),
        Some((RemoteKind::Script, ScriptDialect::QuantumultX))
    );
    // With query / fragment: suffix determination ignores query
    assert_eq!(
        detect_resource_from_url("https://example.com/rules.sgmodule?token=abc&x=1"),
        Some((RemoteKind::Snippet, ScriptDialect::Surge))
    );
    assert_eq!(
        detect_resource_from_url("https://example.com/script.js?token=abc#frag"),
        Some((RemoteKind::Script, ScriptDialect::QuantumultX))
    );
    // Case insensitive
    assert_eq!(
        detect_resource_from_url("https://example.com/RULES.SGMODULE"),
        Some((RemoteKind::Snippet, ScriptDialect::Surge))
    );
    // No suffix / other suffix / empty string -> None
    assert_eq!(detect_resource_from_url("https://example.com/rules"), None);
    assert_eq!(
        detect_resource_from_url("https://example.com/rules.txt"),
        None
    );
    assert_eq!(detect_resource_from_url(""), None);
    // Trailing slash still uses last segment filename for suffix (directory-style URL treated as fragment)
    assert_eq!(
        detect_resource_from_url("https://example.com/rules.conf/"),
        Some((RemoteKind::Snippet, ScriptDialect::QuantumultX))
    );
}

#[tokio::test]
async fn import_content_fills_script_sources_and_merges_surge_sgmodule() {
    let base = spawn_import_server().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let content = format!(
        "#!name=CamScanner-VIP-Unlock\n\
             #!desc=CamScanner - Phone Scanner Unlock Gold Member\n\
             #!date=2026-01-21\n\
             #!category=BOBO Premium\n\
             #!author=AuthorName\n\
             #!icon=https://example.com/CamScanner.png\n\
             #!openUrl=https://apps.apple.com/app/id388627783\n\
             \n\
             [Script]\n\
             CamScanner-VIP-Unlock = type=http-response, pattern=https:\\/\\/api-cs\\.intsig\\.net\\/purchase\\/cs\\/query_property, script-path={base}/camscanner.js, requires-body=true, max-size=-1, timeout=60\n\
             \n\
             [MITM]\n\
             hostname = %APPEND% api-cs.intsig.net\n"
    );

    let summary = manager
        .import_content(&content, ScriptDialect::Surge)
        .await
        .unwrap();
    assert_eq!(summary.rewrites, 0);
    assert_eq!(
        summary.scripts, 1,
        "script source backfilled, merge no longer skips"
    );
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.hostnames, 1);
    assert_eq!(summary.meta.name.as_deref(), Some("CamScanner-VIP-Unlock"));
    assert_eq!(
        summary.meta.open_url.as_deref(),
        Some("https://apps.apple.com/app/id388627783")
    );
    assert!(
        !summary
            .warnings
            .iter()
            .any(|w| w.contains("source not fetched")),
        "should not have source not fetched warning: {:?}",
        summary.warnings
    );

    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.scripts.len(), 1);
    assert_eq!(merged.scripts[0].source, "const camscanner = 1;");
    assert_eq!(merged.scripts[0].name, "CamScanner-VIP-Unlock");
    assert_eq!(merged.hostnames, vec!["api-cs.intsig.net".to_string()]);
}

#[tokio::test]
async fn import_content_records_warning_on_script_fetch_failure_and_keeps_rewrites() {
    let base = spawn_import_server().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let content = format!(
        "#!name=FailedImport\n\
             [rewrite_local]\n\
             ^https?://api-cs.intsig.net/purchase script-response-body {base}/missing.js\n\
             ^https?://example.com/ url-and-header https://target.example.com/\n\
             [mitm]\n\
             hostname = *.camscanner.com\n"
    );

    let summary = manager
        .import_content(&content, ScriptDialect::QuantumultX)
        .await
        .unwrap();
    assert_eq!(
        summary.rewrites, 1,
        "rewrite not affected by script fetch failure"
    );
    assert_eq!(summary.scripts, 0, "failed fetch script skipped");
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.hostnames, 1);
    assert_eq!(summary.meta.name.as_deref(), Some("FailedImport"));
    assert!(
        summary.warnings.iter().any(|w| w.contains("missing.js")),
        "should have script fetch failure warning: {:?}",
        summary.warnings
    );

    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.rewrites.len(), 1);
    assert!(merged.scripts.is_empty());
    assert!(merged.task_scripts.is_empty());
    assert_eq!(merged.hostnames, vec!["*.camscanner.com".to_string()]);
}

#[tokio::test]
async fn import_content_handles_qx_conf_sample() {
    let base = spawn_import_server().await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    // QX common 4-segment: `pattern url script-response-body <path>` (extra `url` modifier ignored)
    let content = format!(
        "#!name=CamScanner-QX\n\
             #!desc=QX Rewrite Sample\n\
             [rewrite_local]\n\
             ^https:\\/\\/.*\\.(intsig\\.net|camscanner\\.com) url script-response-body {base}/camscanner.js\n\
             [mitm]\n\
             hostname = *.camscanner.com, *.intsig.net\n"
    );

    let summary = manager
        .import_content(&content, ScriptDialect::QuantumultX)
        .await
        .unwrap();
    assert_eq!(summary.rewrites, 0);
    assert_eq!(
        summary.scripts, 1,
        "script source backfilled, merge no longer skips"
    );
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.hostnames, 2);
    assert_eq!(summary.meta.name.as_deref(), Some("CamScanner-QX"));
    assert!(
        summary.warnings.is_empty(),
        "warnings: {:?}",
        summary.warnings
    );

    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.scripts.len(), 1);
    assert_eq!(merged.scripts[0].name, "hook-0");
    assert_eq!(merged.scripts[0].source, "const camscanner = 1;");
    assert_eq!(
        merged.hostnames,
        vec!["*.camscanner.com".to_string(), "*.intsig.net".to_string()]
    );
}

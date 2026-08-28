use super::*;
use crate::import::ImportedConfig;
use pp_mitm::RewriteKind;
use pp_script::{ScriptDialect, ScriptKind};
use regex::Regex;

#[test]
fn merge_imported_keeps_rewrites_hostnames_and_skips_source_empty_scripts() {
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let imported = ImportedConfig {
        rewrites: vec![pp_mitm::RewriteRule {
            pattern: Regex::new(r"^https?://example\.com/").unwrap(),
            kind: RewriteKind::Reject,
        }],
        scripts: vec![pp_mitm::ScriptRule {
            name: "hook-0".into(),
            kind: ScriptKind::HttpResponse,
            pattern: Regex::new(r"^https?://example\.com/rsp").unwrap(),
            requires_body: true,
            max_size: 131072,
            source: String::new(),
            argument: None,
        }],
        script_urls: vec![(
            "hook-0".to_string(),
            "https://example.com/hook.js".to_string(),
        )],
        task_scripts: vec![(
            pp_script::TaskScript {
                name: "checkin".into(),
                cron_expr: "0 0 9 * * *".into(),
                source: String::new(),
                dialect: ScriptDialect::QuantumultX,
                enabled: true,
            },
            "https://example.com/task.js".to_string(),
        )],
        hostnames: vec!["*.example.com".to_string()],
        warnings: vec!["parse deviation".to_string()],
        ..Default::default()
    };

    let summary = manager.merge_imported(&imported).unwrap();
    // Rewrite/hostname merged; source-empty scripts and tasks skipped with warning
    assert_eq!(summary.rewrites, 1);
    assert_eq!(summary.hostnames, 1);
    assert_eq!(summary.scripts, 0);
    assert_eq!(summary.tasks, 0);
    assert!(summary.warnings.iter().any(|w| w.contains("hook-0")));
    assert!(summary.warnings.iter().any(|w| w.contains("checkin")));
    assert!(
        summary
            .warnings
            .iter()
            .any(|w| w.contains("parse deviation"))
    );

    // Cache written to remote_cache/imported.json and read by load_cached
    assert!(dir.path().join("remote_cache/imported.json").exists());
    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.rewrites.len(), 1);
    assert!(merged.scripts.is_empty());
    assert!(merged.task_scripts.is_empty());
    assert_eq!(merged.hostnames, vec!["*.example.com".to_string()]);
}

#[test]
fn merge_imported_appends_to_existing_import_cache() {
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());

    let first = ImportedConfig {
        rewrites: vec![pp_mitm::RewriteRule {
            pattern: Regex::new(r"^https?://a\.com/").unwrap(),
            kind: RewriteKind::Reject,
        }],
        scripts: vec![],
        script_urls: vec![],
        task_scripts: vec![],
        hostnames: vec!["a.example.com".to_string()],
        warnings: vec![],
        ..Default::default()
    };
    manager.merge_imported(&first).unwrap();

    let second = ImportedConfig {
        rewrites: vec![pp_mitm::RewriteRule {
            pattern: Regex::new(r"^https?://b\.com/").unwrap(),
            kind: RewriteKind::UrlRewrite {
                target: "https://c.com/$1".into(),
            },
        }],
        scripts: vec![],
        script_urls: vec![],
        task_scripts: vec![],
        hostnames: vec!["b.example.com".to_string()],
        warnings: vec![],
        ..Default::default()
    };
    manager.merge_imported(&second).unwrap();

    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.rewrites.len(), 2);
    assert_eq!(merged.hostnames.len(), 2);
    assert!(
        merged
            .hostnames
            .iter()
            .any(|h| h == "a.example.com" || h == "b.example.com")
    );
}

#[test]
fn merge_imported_skips_source_empty_scripts_and_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let imported = ImportedConfig {
        rewrites: vec![],
        scripts: vec![pp_mitm::ScriptRule {
            name: "empty-script".into(),
            kind: ScriptKind::HttpResponse,
            pattern: Regex::new(r"^https?://example\.com/").unwrap(),
            requires_body: true,
            max_size: 131072,
            source: String::new(),
            argument: None,
        }],
        script_urls: vec![],
        task_scripts: vec![(
            pp_script::TaskScript {
                name: "empty-task".into(),
                cron_expr: "0 0 9 * * *".into(),
                source: String::new(),
                dialect: ScriptDialect::QuantumultX,
                enabled: true,
            },
            String::new(),
        )],
        hostnames: vec![],
        warnings: vec![],
        ..Default::default()
    };

    let summary = manager.merge_imported(&imported).unwrap();
    assert_eq!(summary.scripts, 0);
    assert_eq!(summary.tasks, 0);
    // Script with empty source but no script_urls does not generate warning (zip loop doesn't run)
    assert_eq!(summary.warnings.len(), 1);
    assert!(summary.warnings.iter().any(|w| w.contains("empty-task")));

    let merged = manager.load_cached().unwrap();
    assert!(merged.scripts.is_empty());
    assert!(merged.task_scripts.is_empty());
}

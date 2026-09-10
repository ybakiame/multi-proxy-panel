//! Rule set download / custom rule set tests (split out of `ruleset.rs` to stay
//! within the business-file size gate; see `.agents/rules/code-organization.md`).

use super::*;

fn custom(id: &str, tag: &str, source: CustomRuleSetSource) -> CustomRuleSet {
    CustomRuleSet {
        id: id.to_string(),
        name: String::new(),
        tag: tag.to_string(),
        source,
        last_updated: 0,
        remote_updated_at: 0,
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
    );
    let switched = custom(
        "switched",
        "switched-tag",
        CustomRuleSetSource::Manual {
            content: "manual-content".to_string(),
        },
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

/// 更新链路：覆盖**全部** custom Remote（无 enabled 概念），同步 await 下载，
/// 成功刷新 `last_updated` 并落盘 cache 文件；失败/手动条目 best-effort 跳过。
/// 同时覆盖 HEAD 智能跳过：旧 `Last-Modified` 跳过、新值下载、无头直接下载。
#[tokio::test]
async fn update_custom_remotes_covers_all_remote_with_smart_skip() {
    use axum::http::{StatusCode, header};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        // 新内容：HEAD 带 Last-Modified，GET 返回 body。
        .route(
            "/bin",
            axum::routing::get(|| async { "binary-body" }).head(|| async {
                (
                    [(header::LAST_MODIFIED, "Sun, 06 Nov 1994 08:49:37 GMT")],
                    "",
                )
            }),
        )
        // 旧内容：HEAD 的 Last-Modified 早于本地 last_updated → 跳过下载。
        .route(
            "/stale",
            axum::routing::head(|| async {
                (
                    [(header::LAST_MODIFIED, "Sun, 06 Nov 1994 08:49:37 GMT")],
                    "",
                )
            }),
        )
        // 无 Last-Modified 头：HEAD 成功但无头 → 直接下载，remote_updated_at 归 0。
        .route(
            "/noheader",
            axum::routing::get(|| async { "src-body" }).head(|| async { StatusCode::OK }),
        )
        // HEAD 失败（404）→ 该条失败计数。
        .route(
            "/missing",
            axum::routing::get(|| async { StatusCode::NOT_FOUND })
                .head(|| async { StatusCode::NOT_FOUND }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());

    let mut ovr = LocalOverride::default();
    // 从未下载 + 远端有新内容 → 下载。
    ovr.custom_rule_sets.push(custom(
        "c-ok",
        "ok-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/bin"),
            format: RuleSetFormat::Binary,
        },
    ));
    // 本地 last_updated 晚于远端 Last-Modified → 跳过。
    let mut stale = custom(
        "c-stale",
        "stale-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/stale"),
            format: RuleSetFormat::Source,
        },
    );
    stale.last_updated = 2_000_000_000;
    ovr.custom_rule_sets.push(stale);
    // 无 Last-Modified 头 → 下载。
    ovr.custom_rule_sets.push(custom(
        "c-noheader",
        "noheader-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/noheader"),
            format: RuleSetFormat::Source,
        },
    ));
    // HEAD 失败 → failed。
    ovr.custom_rule_sets.push(custom(
        "c-fail",
        "fail-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/missing"),
            format: RuleSetFormat::Source,
        },
    ));
    // Manual → 不计入。
    ovr.custom_rule_sets.push(custom(
        "c-manual",
        "manual-tag",
        CustomRuleSetSource::Manual {
            content: "[]".to_string(),
        },
    ));

    let outcome = mgr.update_custom_remotes(&mut ovr).await.unwrap();
    assert_eq!(
        outcome,
        RuleSetUpdateOutcome {
            updated: 2,
            skipped: 1,
            failed: 1,
        }
    );

    // c-ok：下载 + last_updated / remote_updated_at 刷新。
    let bin = mgr.custom_rule_set_file_path("c-ok", RuleSetFormat::Binary);
    assert_eq!(std::fs::read_to_string(&bin).unwrap(), "binary-body");
    assert!(ovr.custom_rule_sets[0].last_updated > 0);
    assert_eq!(ovr.custom_rule_sets[0].remote_updated_at, 784_111_777);

    // c-stale：跳过，remote_updated_at 刷新但 last_updated 不变、无文件写入。
    assert_eq!(ovr.custom_rule_sets[1].last_updated, 2_000_000_000);
    assert_eq!(ovr.custom_rule_sets[1].remote_updated_at, 784_111_777);
    assert!(
        !mgr.custom_rule_set_file_path("c-stale", RuleSetFormat::Source)
            .exists()
    );

    // c-noheader：下载，remote_updated_at 归 0。
    assert_eq!(ovr.custom_rule_sets[2].remote_updated_at, 0);
    assert!(ovr.custom_rule_sets[2].last_updated > 0);

    // c-fail / c-manual：未写文件、时间戳不变。
    assert!(
        !mgr.custom_rule_set_file_path("c-fail", RuleSetFormat::Source)
            .exists()
    );
    assert!(
        !mgr.custom_rule_set_file_path("c-manual", RuleSetFormat::Source)
            .exists()
    );
    assert_eq!(ovr.custom_rule_sets[3].last_updated, 0);
    assert_eq!(ovr.custom_rule_sets[4].last_updated, 0);
}

/// 单条更新（智能跳过）：旧远端跳过且不改 `last_updated`；新远端下载并刷新两者；
/// Manual 返回 Skipped。
#[tokio::test]
async fn update_single_rule_set_smart_skip() {
    use axum::http::{StatusCode, header};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/dated",
            axum::routing::get(|| async { "body" }).head(|| async {
                (
                    [(header::LAST_MODIFIED, "Sun, 06 Nov 1994 08:49:37 GMT")],
                    "",
                )
            }),
        )
        .route(
            "/missing",
            axum::routing::get(|| async { StatusCode::NOT_FOUND })
                .head(|| async { StatusCode::NOT_FOUND }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());

    // 远端未更新（Last-Modified 早于本地 last_updated）→ 跳过。
    let mut stale = custom(
        "s",
        "s-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/dated"),
            format: RuleSetFormat::Source,
        },
    );
    stale.last_updated = 2_000_000_000;
    assert_eq!(
        mgr.update_custom_rule_set(&mut stale).await,
        RuleSetUpdateStatus::Skipped
    );
    assert_eq!(stale.remote_updated_at, 784_111_777);
    assert_eq!(stale.last_updated, 2_000_000_000);
    assert!(
        !mgr.custom_rule_set_file_path("s", RuleSetFormat::Source)
            .exists()
    );

    // 远端已更新 → 下载 + 刷新。
    let mut fresh = custom(
        "f",
        "f-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/dated"),
            format: RuleSetFormat::Source,
        },
    );
    fresh.last_updated = 1_000;
    assert_eq!(
        mgr.update_custom_rule_set(&mut fresh).await,
        RuleSetUpdateStatus::Updated
    );
    assert_eq!(fresh.remote_updated_at, 784_111_777);
    assert!(fresh.last_updated > 784_111_777);
    let file = mgr.custom_rule_set_file_path("f", RuleSetFormat::Source);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "body");

    // HEAD 失败 → Failed。
    let mut failing = custom(
        "x",
        "x-tag",
        CustomRuleSetSource::Remote {
            url: format!("http://{addr}/missing"),
            format: RuleSetFormat::Source,
        },
    );
    assert_eq!(
        mgr.update_custom_rule_set(&mut failing).await,
        RuleSetUpdateStatus::Failed
    );

    // Manual → Skipped。
    let mut manual = custom(
        "m",
        "m-tag",
        CustomRuleSetSource::Manual {
            content: "[]".to_string(),
        },
    );
    assert_eq!(
        mgr.update_custom_rule_set(&mut manual).await,
        RuleSetUpdateStatus::Skipped
    );
}

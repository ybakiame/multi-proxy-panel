//! StatsStore 集成测试（临时目录 SQLite）。

use super::store::StatsStore;
use super::{DailyQuery, DailySort, RecordsQuery, StatDelta};

fn delta(target: &str, outbound: &str, up: u64, down: u64, new_conn: bool) -> StatDelta {
    StatDelta {
        target: target.to_string(),
        destination_ip: "1.2.3.4".to_string(),
        rule: "DOMAIN-SUFFIX".to_string(),
        rule_payload: "example.com".to_string(),
        outbound: outbound.to_string(),
        upload: up,
        download: down,
        new_conn,
        seen_at: 1_700_000_000,
    }
}

fn closed_conn(
    target: &str,
    outbound: &str,
    up: u64,
    down: u64,
) -> crate::connections::ConnectionView {
    crate::connections::ConnectionView {
        id: "c-1".to_string(),
        host: target.to_string(),
        target: target.to_string(),
        destination_ip: "1.2.3.4".to_string(),
        outbound: outbound.to_string(),
        network: "tcp".to_string(),
        chain: format!("Proxy → {outbound}"),
        rule: "DOMAIN-SUFFIX".to_string(),
        rule_payload: "example.com".to_string(),
        upload: up,
        download: down,
        start: 1_700_000_000,
    }
}

#[tokio::test]
async fn open_is_idempotent_and_summary_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = StatsStore::open(dir.path()).await.unwrap();
    // 二次打开（schema 已存在）不报错。
    let store2 = StatsStore::open(dir.path()).await.unwrap();
    drop(store2);

    let summary = store.today_summary("2026-01-01").await.unwrap();
    assert_eq!(summary.upload_bytes, 0);
    assert_eq!(summary.download_bytes, 0);
    assert_eq!(summary.connection_count, 0);
    assert_eq!(summary.target_count, 0);
}

#[tokio::test]
async fn apply_batch_accumulates_deltas_and_counts() {
    let dir = tempfile::tempdir().unwrap();
    let store = StatsStore::open(dir.path()).await.unwrap();
    let date = "2026-01-02";

    store
        .apply_batch(date, &[delta("a.com", "node-1", 100, 200, true)], &[])
        .await
        .unwrap();
    store
        .apply_batch(date, &[delta("a.com", "node-1", 50, 60, false)], &[])
        .await
        .unwrap();
    // 另一目标 + 直连出站。
    store
        .apply_batch(date, &[delta("b.com", "direct", 7, 8, true)], &[])
        .await
        .unwrap();

    let summary = store.today_summary(date).await.unwrap();
    assert_eq!(summary.upload_bytes, 157);
    assert_eq!(summary.download_bytes, 268);
    assert_eq!(summary.proxied_upload_bytes, 150);
    assert_eq!(summary.proxied_download_bytes, 260);
    assert_eq!(summary.connection_count, 2);
    assert_eq!(summary.target_count, 2);

    // 查询：默认按总量降序。
    let rows = store
        .query_daily(&DailyQuery::default(), date)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].target, "a.com");
    assert_eq!(rows[0].upload_bytes, 150);
    assert_eq!(rows[0].conn_count, 1);
    assert_eq!(rows[1].target, "b.com");

    // 搜索过滤。
    let rows = store
        .query_daily(
            &DailyQuery {
                search: Some("b.com".into()),
                ..Default::default()
            },
            date,
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].target, "b.com");

    // 排序：按上行升序。
    let rows = store
        .query_daily(
            &DailyQuery {
                sort: DailySort::Upload,
                desc: Some(false),
                ..Default::default()
            },
            date,
        )
        .await
        .unwrap();
    assert_eq!(rows[0].target, "b.com");

    // 出站精确过滤。
    let rows = store
        .query_daily(
            &DailyQuery {
                outbound: Some("direct".into()),
                ..Default::default()
            },
            date,
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].target, "b.com");
}

#[tokio::test]
async fn records_roundtrip_search_and_clear() {
    let dir = tempfile::tempdir().unwrap();
    let store = StatsStore::open(dir.path()).await.unwrap();
    let date = "2026-01-03";

    store
        .apply_batch(
            date,
            &[delta("a.com", "node-1", 1, 2, true)],
            &[closed_conn("a.com", "node-1", 10, 20)],
        )
        .await
        .unwrap();

    let records = store.query_records(&RecordsQuery::default()).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].target, "a.com");
    assert_eq!(records[0].upload, 10);
    assert_eq!(records[0].download, 20);
    assert!(records[0].ended_at > 0);

    let hit = store
        .query_records(&RecordsQuery {
            search: Some("node-1".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(hit.len(), 1);
    let miss = store
        .query_records(&RecordsQuery {
            search: Some("nothing-matches".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(miss.is_empty());

    store.clear().await.unwrap();
    assert!(
        store
            .query_records(&RecordsQuery::default())
            .await
            .unwrap()
            .is_empty()
    );
    let summary = store.today_summary(date).await.unwrap();
    assert_eq!(summary.upload_bytes, 0);
}

#[tokio::test]
async fn apply_batch_skips_empty_deltas() {
    let dir = tempfile::tempdir().unwrap();
    let store = StatsStore::open(dir.path()).await.unwrap();
    store
        .apply_batch(
            "2026-01-04",
            &[delta("idle.com", "node-1", 0, 0, false)],
            &[],
        )
        .await
        .unwrap();
    let summary = store.today_summary("2026-01-04").await.unwrap();
    assert_eq!(summary.connection_count, 0);
    assert_eq!(summary.target_count, 0);
}

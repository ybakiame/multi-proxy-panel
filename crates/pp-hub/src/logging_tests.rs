use super::*;
use sea_orm::EntityTrait;

#[tokio::test]
async fn buffers_startup_and_drains_redacted_application_events() {
    let db = crate::integration_tests::setup_db().await;
    let (sender, receiver) = mpsc::channel(CAPACITY);
    let dropped = Arc::new(AtomicU64::new(0));
    let subscriber = tracing_subscriber::registry().with(DatabaseLayer {
        sender,
        dropped: dropped.clone(),
    });
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "proxy_panel_hub", password = "hidden", node = 42_u64, "startup token=private");
        tracing::debug!(target: "proxy_panel_hub", "debug excluded");
        tracing::info!(target: "sqlx", "SQL excluded");
        tracing::warn!(target: "proxy_panel_hub::logging", "writer excluded");
    });
    PendingLogs { receiver, dropped }
        .start(db.clone())
        .shutdown()
        .await;
    let records = system_log::Entity::find().all(&db).await.expect("logs");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].level, "info");
    assert!(!records[0].message.contains("private"));
    let fields = records[0].metadata.as_ref().expect("fields");
    assert_eq!(fields["password"], "[REDACTED]");
    assert_eq!(fields["node"], "42");
}

#[test]
fn overflowing_queue_does_not_block_application() {
    let (sender, mut receiver) = mpsc::channel(1);
    let dropped = Arc::new(AtomicU64::new(0));
    let subscriber = tracing_subscriber::registry().with(DatabaseLayer {
        sender,
        dropped: dropped.clone(),
    });
    tracing::subscriber::with_default(subscriber, || {
        for _ in 0..10 {
            tracing::info!(target: "proxy_panel_hub", "event");
        }
    });
    assert_eq!(dropped.load(Ordering::Relaxed), 9);
    assert!(receiver.try_recv().is_ok());
}

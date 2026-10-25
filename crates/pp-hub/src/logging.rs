//! Nonblocking persistence of bounded application tracing records.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use pp_common::logging::LogVisitor;
use pp_db::entities::system_log;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use tokio::sync::{mpsc, oneshot};
use tracing_subscriber::prelude::*;

const CAPACITY: usize = 512;

struct Record {
    level: String,
    source: String,
    visitor: LogVisitor,
    timestamp: chrono::DateTime<chrono::Utc>,
}

struct DatabaseLayer {
    sender: mpsc::Sender<Record>,
    dropped: Arc<AtomicU64>,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for DatabaseLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let source = metadata.target();
        if *metadata.level() > tracing::Level::INFO
            || !(source.starts_with("proxy_panel_hub") || source.starts_with("pp_"))
            || source.starts_with("proxy_panel_hub::logging")
        {
            return;
        }
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            visitor.message = format!("event at {source}");
        }
        let record = Record {
            level: metadata.level().as_str().to_lowercase(),
            source: source.into(),
            visitor,
            timestamp: chrono::Utc::now(),
        };
        if self.sender.try_send(record).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub struct PendingLogs {
    receiver: mpsc::Receiver<Record>,
    dropped: Arc<AtomicU64>,
}

/// Initialize console and persistence layers before startup messages are emitted.
pub fn init() -> PendingLogs {
    let (sender, receiver) = mpsc::channel(CAPACITY);
    let dropped = Arc::new(AtomicU64::new(0));
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "proxy_panel_hub=info,pp_core=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(DatabaseLayer {
            sender,
            dropped: dropped.clone(),
        })
        .init();
    PendingLogs { receiver, dropped }
}

pub struct LogWriter {
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl PendingLogs {
    pub fn start(self, db: DatabaseConnection) -> LogWriter {
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(persist(self.receiver, self.dropped, db, stopped));
        LogWriter { stop, task }
    }
}

impl LogWriter {
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(());
        if tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
            eprintln!("Hub log persistence shutdown timed out");
        }
    }
}

async fn persist(
    mut receiver: mpsc::Receiver<Record>,
    dropped: Arc<AtomicU64>,
    db: DatabaseConnection,
    mut stopped: oneshot::Receiver<()>,
) {
    let mut draining = false;
    loop {
        let record = if draining {
            receiver.recv().await
        } else {
            tokio::select! {
                record = receiver.recv() => record,
                _ = &mut stopped => { receiver.close(); draining = true; continue; }
            }
        };
        let Some(record) = record else {
            break;
        };
        let active = system_log::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            level: Set(record.level),
            source: Set(record.source),
            message: Set(record.visitor.message),
            metadata: Set(Some(serde_json::json!(record.visitor.fields))),
            created_at: Set(record.timestamp.into()),
        };
        // Never trace the writer's own DB errors: that would create a feedback loop.
        match tokio::time::timeout(Duration::from_secs(2), active.insert(&db)).await {
            Ok(Ok(_)) => {}
            _ => eprintln!("Hub log persistence failed; one record dropped"),
        }
        let count = dropped.swap(0, Ordering::Relaxed);
        if count > 0 {
            eprintln!("Hub log persistence queue dropped {count} records");
        }
    }
}

#[cfg(test)]
#[path = "logging_tests.rs"]
mod tests;

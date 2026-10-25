use pp_proto::{CoreStatusReport, LogBatch, LogEntry};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

const MAX_BUFFERED_LOGS: usize = 1000;

/// Shared log buffer used by the tracing layer and the gRPC log sender.
pub struct AgentLogger {
    tx: mpsc::Sender<LogEntry>,
    rx: Arc<Mutex<mpsc::Receiver<LogEntry>>>,
}

impl AgentLogger {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<LogEntry>(MAX_BUFFERED_LOGS);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn sender(&self) -> mpsc::Sender<LogEntry> {
        self.tx.clone()
    }

    pub fn receiver(&self) -> Arc<Mutex<mpsc::Receiver<LogEntry>>> {
        self.rx.clone()
    }
}

impl Default for AgentLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// A tracing layer that forwards log records to the given channel.
pub struct GrpcLogLayer {
    tx: mpsc::Sender<LogEntry>,
}

impl GrpcLogLayer {
    pub fn new(tx: mpsc::Sender<LogEntry>) -> Self {
        Self { tx }
    }
}

impl<S> tracing_subscriber::Layer<S> for GrpcLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = event.metadata().level().as_str().to_lowercase();
        let target = event.metadata().target().to_string();
        let mut visitor = pp_common::logging::LogVisitor::default();
        event.record(&mut visitor);
        let message = if visitor.message.is_empty() {
            format!("event at {target}")
        } else {
            visitor.message
        };
        let fields = visitor.fields;
        let entry = LogEntry {
            timestamp: chrono::Utc::now().timestamp(),
            level,
            target,
            message,
            fields,
        };

        // Drop silently if the channel is full to avoid blocking.
        let _ = self.tx.try_send(entry);
    }
}

/// Build a LogBatch from up to `limit` entries in the receiver.
pub async fn collect_logs(rx: &mut mpsc::Receiver<LogEntry>, limit: usize) -> LogBatch {
    let mut entries = Vec::new();
    while let Ok(Some(entry)) =
        tokio::time::timeout(std::time::Duration::from_millis(10), rx.recv()).await
    {
        entries.push(entry);
        if entries.len() >= limit {
            break;
        }
    }
    LogBatch { entries }
}

/// Collect core status from the supervisor and return a report.
pub async fn collect_core_status(supervisor: &pp_core::CoreSupervisor) -> Vec<CoreStatusReport> {
    let mut reports = Vec::new();
    for core_type in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if let Some(manager) = supervisor.get(core_type).await {
            let running = manager.is_running().await;
            let version = manager.version().await.unwrap_or_default();
            let uptime_sec = manager.uptime_secs().await.unwrap_or_default();
            let active_inbounds = manager.active_inbounds().await.unwrap_or_default();
            let last_error = manager.last_error().await.unwrap_or_default();

            reports.push(CoreStatusReport {
                core_type: core_type.to_string(),
                version,
                running,
                uptime_sec: uptime_sec as i64,
                active_inbounds,
                last_error,
            });
        }
    }
    reports
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn forwards_filtered_redacted_records_with_fields() {
        let (tx, mut rx) = mpsc::channel(10);
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                "proxy_panel_agent=info,pp_core=info",
            ))
            .with(GrpcLogLayer::new(tx));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "pp_core", password = "private", retries = 2_u64, "core token=hidden");
            tracing::debug!(target: "pp_core", "excluded");
        });
        let batch = collect_logs(&mut rx, 10).await;
        assert_eq!(batch.entries.len(), 1);
        assert_eq!(batch.entries[0].fields["password"], "[REDACTED]");
        assert_eq!(batch.entries[0].fields["retries"], "2");
        assert!(!batch.entries[0].message.contains("hidden"));
    }
}

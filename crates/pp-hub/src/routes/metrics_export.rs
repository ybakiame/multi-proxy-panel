//! Prometheus metrics endpoint.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use metrics_exporter_prometheus::PrometheusHandle;

use crate::state::AppState;

/// Shared metrics handle for Prometheus exposition.
pub struct MetricsHandle {
    pub handle: PrometheusHandle,
}

impl MetricsHandle {
    pub fn new() -> Self {
        use metrics_exporter_prometheus::PrometheusBuilder;

        let handle = PrometheusBuilder::new()
            .install_recorder()
            .expect("failed to install Prometheus recorder");

        Self { handle }
    }
}

/// GET /metrics — Expose Prometheus metrics.
pub async fn prometheus_metrics(
    State(state): State<std::sync::Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if let Some(metrics) = &state.metrics_handle {
        let prometheus_text = metrics.handle.render();
        Ok((
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            prometheus_text,
        ))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Record an HTTP request metric.
#[allow(dead_code)]
pub fn record_http_request(method: &str, path: &str, status: u16) {
    metrics::counter!(
        "proxypanel_http_requests_total",
        "method" => method.to_string(),
        "path" => path.to_string(),
        "status" => status.to_string(),
    )
    .increment(1);
}

/// Record an agent connection event.
#[allow(dead_code)]
pub fn record_agent_connection(event: &str) {
    metrics::counter!(
        "proxypanel_agent_connections_total",
        "event" => event.to_string(),
    )
    .increment(1);
}

/// Record a gRPC message received.
#[allow(dead_code)]
pub fn record_grpc_message(message_type: &str) {
    metrics::counter!(
        "proxypanel_grpc_messages_total",
        "type" => message_type.to_string(),
    )
    .increment(1);
}

/// Set the number of active agents.
#[allow(dead_code)]
pub fn set_active_agents(count: u64) {
    metrics::gauge!("proxypanel_active_agents").set(count as f64);
}

/// Set the number of active clients.
#[allow(dead_code)]
pub fn set_active_clients(count: u64) {
    metrics::gauge!("proxypanel_active_clients").set(count as f64);
}

/// Set the number of active nodes.
#[allow(dead_code)]
pub fn set_active_nodes(count: u64) {
    metrics::gauge!("proxypanel_active_nodes").set(count as f64);
}

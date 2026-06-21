//! pp-proto — gRPC protobuf definitions and tonic generated code.

pub mod hub_agent {
    tonic::include_proto!("proxypanel");
}

pub mod xray_stats {
    tonic::include_proto!("xray.stats");
}

pub mod singbox_daemon {
    tonic::include_proto!("daemon");
}

pub use hub_agent::hub_agent_client::HubAgentClient;
pub use hub_agent::hub_agent_server::{HubAgent, HubAgentServer};
pub use hub_agent::*;
pub use xray_stats::stats_service_client::StatsServiceClient;
pub use xray_stats::*;
pub use singbox_daemon::started_service_client::StartedServiceClient;
pub use singbox_daemon::managed_service_client::ManagedServiceClient;
pub use singbox_daemon::*;

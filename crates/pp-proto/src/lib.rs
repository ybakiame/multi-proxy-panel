//! pp-proto — gRPC protobuf definitions and tonic generated code.

// Generated tonic code returns bare `tonic::Status` in Results, which newer
// clippy (result_large_err) flags; we can't change codegen, so allow it here.
#![allow(clippy::result_large_err)]

pub mod hub_agent {
    tonic::include_proto!("proxypanel");
}

pub mod singbox_daemon {
    tonic::include_proto!("daemon");
}

pub use hub_agent::hub_agent_client::HubAgentClient;
pub use hub_agent::hub_agent_server::{HubAgent, HubAgentServer};
pub use hub_agent::*;
pub use singbox_daemon::managed_service_client::ManagedServiceClient;
pub use singbox_daemon::started_service_client::StartedServiceClient;
pub use singbox_daemon::*;

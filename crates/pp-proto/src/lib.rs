//! pp-proto — gRPC protobuf definitions and tonic generated code.

// Generated tonic code returns bare `tonic::Status` in Results and uses
// must_use on generated futures; we can't change codegen, so allow both lints here.
#![allow(clippy::result_large_err)]
#![allow(clippy::double_must_use)]

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

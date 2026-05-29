//! pp-proto — gRPC protobuf definitions and tonic generated code.

pub mod hub_agent {
    tonic::include_proto!("proxypanel");
}

pub use hub_agent::hub_agent_client::HubAgentClient;
pub use hub_agent::hub_agent_server::{HubAgent, HubAgentServer};
pub use hub_agent::*;

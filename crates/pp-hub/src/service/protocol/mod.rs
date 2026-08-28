//! Protocol service — configuration generation and push orchestration.
//!
//! Re-exports preserve the original public API surface.

mod config_gen;
mod push;
mod relay;

pub use config_gen::generate_node_config;
pub use push::{
    UPDATE_TYPE_CONFIG, UPDATE_TYPE_CORE, clear_pending, core_build_id_of, effective_core_version,
    mark_pending, nodes_using_config, nodes_with_bindings, push_node_config, push_version_of,
};

pub use sea_orm_migration::prelude::*;

pub use sea_orm_migration::MigratorTrait;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250101_000001_create_initial_tables::Migration),
            Box::new(m20250630_000001_create_node_groups::Migration),
            Box::new(m20250630_000002_create_online_sessions::Migration),
            Box::new(m20250701_000001_add_client_reset_strategy::Migration),
            Box::new(m20250701_000002_add_node_usage_coefficient_and_node_user_usage::Migration),
            Box::new(m20250701_000003_add_client_max_devices::Migration),
            Box::new(m20250701_000004_create_api_keys::Migration),
            Box::new(m20250701_000005_create_webhooks::Migration),
            Box::new(m20250702_000001_add_constraints_and_indexes::Migration),
            Box::new(m20250703_000001_add_on_hold_and_node_parent::Migration),
            Box::new(m20250703_000002_create_inbound_hosts::Migration),
            Box::new(m20250703_000003_add_rate_to_usage_records::Migration),
            Box::new(m20250710_000001_remove_subscription_template_id::Migration),
            Box::new(m20250710_000002_add_protocol_config_core_version::Migration),
            Box::new(m20250710_000003_binding_level_groups::Migration),
            Box::new(m20250711_000001_add_node_domain::Migration),
            Box::new(m20250711_000002_subscription_template_text::Migration),
            Box::new(m20250711_000003_create_agent_logs::Migration),
            Box::new(m20250711_000004_cleanup_protocols_and_template_flags::Migration),
            Box::new(m20250712_000001_fix_subscription_template_blob_ids::Migration),
            Box::new(m20260719_000001_create_core_versions::Migration),
            Box::new(m20260720_000001_create_certificates::Migration),
            Box::new(m20260722_000001_create_system_meta::Migration),
            Box::new(m20260722_000002_update_core_type_check::Migration),
            Box::new(m20260724_000001_add_core_version_build_info::Migration),
        ]
    }
}

mod m20250101_000001_create_initial_tables;
mod m20250630_000001_create_node_groups;
mod m20250630_000002_create_online_sessions;
mod m20250701_000001_add_client_reset_strategy;
mod m20250701_000002_add_node_usage_coefficient_and_node_user_usage;
mod m20250701_000003_add_client_max_devices;
mod m20250701_000004_create_api_keys;
mod m20250701_000005_create_webhooks;
mod m20250702_000001_add_constraints_and_indexes;
mod m20250703_000001_add_on_hold_and_node_parent;
mod m20250703_000002_create_inbound_hosts;
mod m20250703_000003_add_rate_to_usage_records;
mod m20250710_000001_remove_subscription_template_id;
mod m20250710_000002_add_protocol_config_core_version;
mod m20250710_000003_binding_level_groups;
mod m20250711_000001_add_node_domain;
mod m20250711_000002_subscription_template_text;
mod m20250711_000003_create_agent_logs;
mod m20250711_000004_cleanup_protocols_and_template_flags;
mod m20250712_000001_fix_subscription_template_blob_ids;
mod m20260719_000001_create_core_versions;
mod m20260720_000001_create_certificates;
mod m20260722_000001_create_system_meta;
mod m20260722_000002_update_core_type_check;
mod m20260724_000001_add_core_version_build_info;

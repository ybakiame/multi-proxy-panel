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

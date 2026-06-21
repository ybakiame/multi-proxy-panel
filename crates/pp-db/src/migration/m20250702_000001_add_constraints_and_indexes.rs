use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Foreign keys and unique constraints added after table creation to keep
        // initial migrations clean and avoid cross-migration ordering issues.
        // SQLite does not support ALTER TABLE ADD FOREIGN KEY, so we skip FKs on SQLite.
        if manager.get_database_backend() != sea_orm::DbBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_bindings_node")
                        .from(NodeBindings::Table, NodeBindings::NodeId)
                        .to(Nodes::Table, Nodes::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;

            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_bindings_protocol_config")
                        .from(NodeBindings::Table, NodeBindings::ProtocolConfigId)
                        .to(ProtocolConfigs::Table, ProtocolConfigs::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;

            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_subscriptions_client")
                        .from(Subscriptions::Table, Subscriptions::ClientId)
                        .to(Clients::Table, Clients::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;

            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_subscriptions_template")
                        .from(Subscriptions::Table, Subscriptions::TemplateId)
                        .to(SubscriptionTemplates::Table, SubscriptionTemplates::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;

            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_traffic_records_node")
                        .from(TrafficRecords::Table, TrafficRecords::NodeId)
                        .to(Nodes::Table, Nodes::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;

            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_traffic_records_client")
                        .from(TrafficRecords::Table, TrafficRecords::ClientId)
                        .to(Clients::Table, Clients::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;

            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_host_metrics_node")
                        .from(HostMetrics::Table, HostMetrics::NodeId)
                        .to(Nodes::Table, Nodes::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;
        }

        // Unique constraint to prevent duplicate bindings
        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("idx_node_bindings_unique")
                    .table(NodeBindings::Table)
                    .col(NodeBindings::NodeId)
                    .col(NodeBindings::ProtocolConfigId)
                    .to_owned(),
            )
            .await?;

        // Unique constraint on subscription token
        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("idx_subscriptions_token")
                    .table(Subscriptions::Table)
                    .col(Subscriptions::Token)
                    .to_owned(),
            )
            .await?;

        // Query indexes for high-cardinality columns
        manager
            .create_index(
                Index::create()
                    .name("idx_traffic_client")
                    .table(TrafficRecords::Table)
                    .col(TrafficRecords::ClientId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_traffic_node")
                    .table(TrafficRecords::Table)
                    .col(TrafficRecords::NodeId)
                    .to_owned(),
            )
            .await?;

        // CHECK constraints for status and type enums.
        // Sea-ORM does not expose a first-class CHECK builder, so we use raw SQL.
        // SQLite does not support ALTER TABLE ADD CONSTRAINT, so we skip CHECKs on SQLite.
        if manager.get_database_backend() != sea_orm::DbBackend::Sqlite {
            for stmt in [
                "ALTER TABLE nodes ADD CONSTRAINT chk_nodes_status CHECK (status IN ('online', 'offline', 'maintenance'))",
                "ALTER TABLE clients ADD CONSTRAINT chk_clients_status CHECK (status IN ('active', 'inactive', 'expired', 'disabled'))",
                "ALTER TABLE protocol_configs ADD CONSTRAINT chk_protocol_configs_type CHECK (protocol_type IN ('vmess', 'vless', 'trojan', 'shadowsocks', 'shadowsocksr', 'hysteria2', 'tuic', 'wireguard', 'vless_reality'))",
                "ALTER TABLE protocol_configs ADD CONSTRAINT chk_protocol_configs_core CHECK (core_type IN ('xray', 'sing-box'))",
            ] {
                manager.get_connection().execute_unprepared(stmt).await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DbBackend::Sqlite {
            manager
                .drop_index(Index::drop().name("idx_node_bindings_unique").table(NodeBindings::Table).to_owned())
                .await?;
            manager
                .drop_index(Index::drop().name("idx_subscriptions_token").table(Subscriptions::Table).to_owned())
                .await?;
            manager
                .drop_index(Index::drop().name("idx_traffic_client").table(TrafficRecords::Table).to_owned())
                .await?;
            manager
                .drop_index(Index::drop().name("idx_traffic_node").table(TrafficRecords::Table).to_owned())
                .await?;
            return Ok(());
        }

        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_bindings_node").table(NodeBindings::Table).to_owned())
            .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_bindings_protocol_config").table(NodeBindings::Table).to_owned())
            .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_subscriptions_client").table(Subscriptions::Table).to_owned())
            .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_subscriptions_template").table(Subscriptions::Table).to_owned())
            .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_traffic_records_node").table(TrafficRecords::Table).to_owned())
            .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_traffic_records_client").table(TrafficRecords::Table).to_owned())
            .await?;
        manager
            .drop_foreign_key(ForeignKey::drop().name("fk_host_metrics_node").table(HostMetrics::Table).to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_node_bindings_unique").table(NodeBindings::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_subscriptions_token").table(Subscriptions::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_traffic_client").table(TrafficRecords::Table).to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_traffic_node").table(TrafficRecords::Table).to_owned())
            .await?;

        for (table, chk) in [
            ("nodes", "chk_nodes_status"),
            ("clients", "chk_clients_status"),
            ("protocol_configs", "chk_protocol_configs_type"),
            ("protocol_configs", "chk_protocol_configs_core"),
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!(
                    "ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}",
                    table, chk
                ))
                .await?;
        }

        Ok(())
    }
}

#[derive(Iden)]
enum Nodes {
    Table,
    Id,
}

#[derive(Iden)]
enum ProtocolConfigs {
    Table,
    Id,
}

#[derive(Iden)]
enum Clients {
    Table,
    Id,
}

#[derive(Iden)]
enum SubscriptionTemplates {
    Table,
    Id,
}

#[derive(Iden)]
enum NodeBindings {
    Table,
    NodeId,
    ProtocolConfigId,
}

#[derive(Iden)]
enum Subscriptions {
    Table,
    ClientId,
    TemplateId,
    Token,
}

#[derive(Iden)]
enum TrafficRecords {
    Table,
    NodeId,
    ClientId,
}

#[derive(Iden)]
enum HostMetrics {
    Table,
    NodeId,
}

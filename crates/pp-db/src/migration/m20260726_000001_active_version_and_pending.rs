use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // a) Add is_active column to core_versions
        manager
            .alter_table(
                Table::alter()
                    .table(CoreVersions::Table)
                    .add_column(
                        ColumnDef::new(CoreVersions::IsActive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // b) Create node_pending_updates table
        manager
            .create_table(
                Table::create()
                    .table(NodePendingUpdates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NodePendingUpdates::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(NodePendingUpdates::NodeId).uuid().not_null())
                    .col(
                        ColumnDef::new(NodePendingUpdates::CoreType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NodePendingUpdates::UpdateType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NodePendingUpdates::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index on (node_id, core_type)
        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("idx_node_pending_updates_node_core")
                    .table(NodePendingUpdates::Table)
                    .col(NodePendingUpdates::NodeId)
                    .col(NodePendingUpdates::CoreType)
                    .to_owned(),
            )
            .await?;

        // c) Preserve active pins from protocol_configs.core_version into
        //    core_versions.is_active before dropping the column.
        //    Uses sea_query via the connection (backend-agnostic).
        let conn = manager.get_connection();
        let backend = manager.get_database_backend();

        let rows = conn
            .query_all(sea_orm::Statement::from_string(
                backend,
                "SELECT core_type, core_version FROM protocol_configs WHERE core_version IS NOT NULL AND core_version != ''".to_owned(),
            ))
            .await?;

        // Group by core_type, keep the greatest version string per group.
        let mut pins: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for row in &rows {
            let core_type: String = row.try_get_by_index(0)?;
            let version: String = row.try_get_by_index(1)?;
            let entry = pins.entry(core_type).or_default();
            if &version > entry {
                *entry = version;
            }
        }

        // Activate matching core_versions rows.
        for (core_type, version) in &pins {
            let escaped_ct = core_type.replace('\'', "''");
            let escaped_ver = version.replace('\'', "''");
            let sql = format!(
                "UPDATE core_versions SET is_active = true WHERE core_type = '{}' AND version = '{}'",
                escaped_ct, escaped_ver,
            );
            conn.execute_unprepared(&sql).await?;
        }

        // Drop the core_version column from protocol_configs.
        manager
            .alter_table(
                Table::alter()
                    .table(ProtocolConfigs::Table)
                    .drop_column(ProtocolConfigs::CoreVersion)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-add core_version column to protocol_configs
        manager
            .alter_table(
                Table::alter()
                    .table(ProtocolConfigs::Table)
                    .add_column(ColumnDef::new(ProtocolConfigs::CoreVersion).string().null())
                    .to_owned(),
            )
            .await?;

        // Drop is_active column from core_versions
        manager
            .alter_table(
                Table::alter()
                    .table(CoreVersions::Table)
                    .drop_column(CoreVersions::IsActive)
                    .to_owned(),
            )
            .await?;

        // Drop node_pending_updates table
        manager
            .drop_table(Table::drop().table(NodePendingUpdates::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum CoreVersions {
    Table,
    IsActive,
}

#[derive(Iden)]
enum NodePendingUpdates {
    Table,
    Id,
    NodeId,
    CoreType,
    UpdateType,
    UpdatedAt,
}

#[derive(Iden)]
enum ProtocolConfigs {
    Table,
    CoreVersion,
}

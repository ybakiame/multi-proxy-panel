use sea_orm_migration::prelude::*;

/// xray support was removed in 0.2.0: narrow the protocol_configs core_type
/// CHECK constraint to the remaining cores. SQLite is skipped (it does not
/// support ALTER TABLE ADD/DROP CONSTRAINT), matching the original migration.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DbBackend::Sqlite {
            return Ok(());
        }
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE protocol_configs DROP CONSTRAINT chk_protocol_configs_core",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE protocol_configs ADD CONSTRAINT chk_protocol_configs_core CHECK (core_type IN ('sing-box', 'mihomo'))",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DbBackend::Sqlite {
            return Ok(());
        }
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE protocol_configs DROP CONSTRAINT chk_protocol_configs_core",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE protocol_configs ADD CONSTRAINT chk_protocol_configs_core CHECK (core_type IN ('xray', 'sing-box'))",
        )
        .await?;
        Ok(())
    }
}

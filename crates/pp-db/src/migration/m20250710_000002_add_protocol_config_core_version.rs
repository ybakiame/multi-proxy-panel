use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ProtocolConfigs::Table)
                    .add_column(ColumnDef::new(ProtocolConfigs::CoreVersion).string().null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
}

#[derive(Iden)]
enum ProtocolConfigs {
    Table,
    CoreVersion,
}

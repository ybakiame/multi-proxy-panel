use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SystemMeta::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SystemMeta::Key)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SystemMeta::Value).text().not_null())
                    .col(
                        ColumnDef::new(SystemMeta::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SystemMeta::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SystemMeta {
    Table,
    Key,
    Value,
    UpdatedAt,
}

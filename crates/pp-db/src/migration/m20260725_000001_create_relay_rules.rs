use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RelayRules::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RelayRules::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .to_owned(),
                    )
                    .col(ColumnDef::new(RelayRules::NodeId).uuid().not_null())
                    .col(ColumnDef::new(RelayRules::ExitBindingId).uuid().not_null())
                    .col(ColumnDef::new(RelayRules::RelayClientId).uuid().not_null())
                    .col(ColumnDef::new(RelayRules::Name).string_len(128).not_null())
                    .col(
                        ColumnDef::new(RelayRules::MatchType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(RelayRules::MatchConfig).json().not_null())
                    .col(
                        ColumnDef::new(RelayRules::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(RelayRules::SortOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RelayRules::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(RelayRules::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RelayRules::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum RelayRules {
    Table,
    Id,
    NodeId,
    ExitBindingId,
    RelayClientId,
    Name,
    MatchType,
    MatchConfig,
    Enabled,
    SortOrder,
    CreatedAt,
    UpdatedAt,
}

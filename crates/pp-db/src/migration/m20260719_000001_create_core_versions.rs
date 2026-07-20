use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CoreVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CoreVersions::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .to_owned(),
                    )
                    .col(ColumnDef::new(CoreVersions::CoreType).string().not_null())
                    .col(ColumnDef::new(CoreVersions::Version).string().not_null())
                    .col(ColumnDef::new(CoreVersions::Channel).string().not_null())
                    .col(
                        ColumnDef::new(CoreVersions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string())
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_core_versions_type_version")
                    .table(CoreVersions::Table)
                    .col(CoreVersions::CoreType)
                    .col(CoreVersions::Version)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CoreVersions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum CoreVersions {
    Table,
    Id,
    CoreType,
    Version,
    Channel,
    CreatedAt,
}

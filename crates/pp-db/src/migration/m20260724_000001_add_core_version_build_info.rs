use sea_orm_migration::prelude::*;

/// Rolling-tag build metadata for core_versions: a prerelease tag such as
/// mihomo's `Prerelease-Alpha` keeps the same version string while the build
/// is replaced upstream, so `published_at`/`commit_sha` are needed to tell
/// builds apart.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CoreVersions::Table)
                    .add_column(
                        ColumnDef::new(CoreVersions::PublishedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CoreVersions::Table)
                    .add_column(
                        ColumnDef::new(CoreVersions::CommitSha)
                            .string_len(64)
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CoreVersions::Table)
                    .drop_column(CoreVersions::PublishedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CoreVersions::Table)
                    .drop_column(CoreVersions::CommitSha)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CoreVersions {
    Table,
    PublishedAt,
    CommitSha,
}

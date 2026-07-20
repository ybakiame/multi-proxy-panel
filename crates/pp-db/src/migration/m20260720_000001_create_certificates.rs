use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Certificates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Certificates::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .to_owned(),
                    )
                    .col(ColumnDef::new(Certificates::NodeId).uuid().not_null())
                    .col(ColumnDef::new(Certificates::Domain).string().not_null())
                    .col(ColumnDef::new(Certificates::Status).string().not_null())
                    .col(
                        ColumnDef::new(Certificates::ChallengeType)
                            .string()
                            .not_null()
                            .default("http-01"),
                    )
                    .col(
                        ColumnDef::new(Certificates::ExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Certificates::LastIssuedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(Certificates::LastError).text().null())
                    .col(
                        ColumnDef::new(Certificates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(Certificates::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_certificates_node_domain")
                    .table(Certificates::Table)
                    .col(Certificates::NodeId)
                    .col(Certificates::Domain)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Certificates::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Certificates {
    Table,
    Id,
    NodeId,
    Domain,
    Status,
    ChallengeType,
    ExpiresAt,
    LastIssuedAt,
    LastError,
    CreatedAt,
    UpdatedAt,
}

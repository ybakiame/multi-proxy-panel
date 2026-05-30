use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add usage_coefficient to nodes table
        manager
            .alter_table(
                Table::alter()
                    .table(Nodes::Table)
                    .add_column(float(Nodes::UsageCoefficient).default(1.0))
                    .to_owned(),
            )
            .await?;

        // Create node_user_usage_records table
        manager
            .create_table(
                Table::create()
                    .table(NodeUserUsageRecords::Table)
                    .if_not_exists()
                    .col(pk_uuid(NodeUserUsageRecords::Id))
                    .col(uuid(NodeUserUsageRecords::NodeId))
                    .col(uuid(NodeUserUsageRecords::ClientId))
                    .col(timestamp(NodeUserUsageRecords::HourBucket))
                    .col(big_integer(NodeUserUsageRecords::UploadBytes))
                    .col(big_integer(NodeUserUsageRecords::DownloadBytes))
                    .col(timestamp(NodeUserUsageRecords::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_user_usage_node")
                    .table(NodeUserUsageRecords::Table)
                    .col(NodeUserUsageRecords::NodeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_user_usage_client")
                    .table(NodeUserUsageRecords::Table)
                    .col(NodeUserUsageRecords::ClientId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_user_usage_hour")
                    .table(NodeUserUsageRecords::Table)
                    .col(NodeUserUsageRecords::HourBucket)
                    .to_owned(),
            )
            .await?;

        // Composite unique index for upsert
        manager
            .create_index(
                Index::create()
                    .name("idx_node_user_usage_unique")
                    .table(NodeUserUsageRecords::Table)
                    .col(NodeUserUsageRecords::NodeId)
                    .col(NodeUserUsageRecords::ClientId)
                    .col(NodeUserUsageRecords::HourBucket)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NodeUserUsageRecords::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Nodes::Table)
                    .drop_column(Nodes::UsageCoefficient)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Nodes {
    Table,
    UsageCoefficient,
}

#[derive(Iden)]
enum NodeUserUsageRecords {
    Table,
    Id,
    NodeId,
    ClientId,
    HourBucket,
    UploadBytes,
    DownloadBytes,
    CreatedAt,
}

fn pk_uuid<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().not_null().primary_key().to_owned()
}
fn uuid<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().not_null().to_owned()
}
fn timestamp<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .timestamp_with_time_zone()
        .not_null()
        .extra("DEFAULT NOW()".to_string())
        .to_owned()
}
fn float<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).float().to_owned()
}
fn big_integer<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).big_integer().not_null().to_owned()
}

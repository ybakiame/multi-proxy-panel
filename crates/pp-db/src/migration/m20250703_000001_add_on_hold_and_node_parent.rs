use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // on_hold_expire_duration_secs: subscription duration in seconds granted on first connect
        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .add_column(big_integer_null(Clients::OnHoldExpireDurationSecs))
                    .to_owned(),
            )
            .await?;

        // on_hold_timeout: absolute deadline by which the user must first connect
        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .add_column(timestamp_null(Clients::OnHoldTimeout))
                    .to_owned(),
            )
            .await?;

        // parent_id: optional self-referencing FK for relay/child node topology
        manager
            .alter_table(
                Table::alter()
                    .table(Nodes::Table)
                    .add_column(uuid_null(Nodes::ParentId))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Nodes::Table)
                    .drop_column(Nodes::ParentId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .drop_column(Clients::OnHoldTimeout)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .drop_column(Clients::OnHoldExpireDurationSecs)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Clients {
    Table,
    OnHoldExpireDurationSecs,
    OnHoldTimeout,
}

#[derive(Iden)]
enum Nodes {
    Table,
    ParentId,
}

fn big_integer_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).big_integer().to_owned()
}

fn timestamp_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).timestamp_with_time_zone().to_owned()
}

fn uuid_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().to_owned()
}

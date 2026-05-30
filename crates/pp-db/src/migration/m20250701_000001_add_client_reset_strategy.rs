use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .add_column(string(Clients::DataLimitResetStrategy).default("no_reset"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .add_column(timestamp_null(Clients::LastTrafficResetTime))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .add_column(big_integer(Clients::AllTimeUsedBytes).default(0))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .drop_column(Clients::DataLimitResetStrategy)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .drop_column(Clients::LastTrafficResetTime)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .drop_column(Clients::AllTimeUsedBytes)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Clients {
    Table,
    DataLimitResetStrategy,
    LastTrafficResetTime,
    AllTimeUsedBytes,
}

fn string<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().not_null().to_owned()
}

fn timestamp_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).timestamp_with_time_zone().to_owned()
}

fn big_integer<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).big_integer().not_null().to_owned()
}

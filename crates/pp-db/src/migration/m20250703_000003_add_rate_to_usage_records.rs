use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // rate: traffic coefficient used when counting traffic for this record
        manager
            .alter_table(
                Table::alter()
                    .table(NodeUserUsageRecords::Table)
                    .add_column(float_not_null(NodeUserUsageRecords::Rate).default(1.0))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(NodeUserUsageRecords::Table)
                    .drop_column(NodeUserUsageRecords::Rate)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum NodeUserUsageRecords {
    Table,
    Rate,
}

fn float_not_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).float().not_null().to_owned()
}

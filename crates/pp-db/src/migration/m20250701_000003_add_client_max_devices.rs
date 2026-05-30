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
                    .add_column(integer_null(Clients::MaxDevices))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_online_sessions_client_ip")
                    .table(ClientOnlineSessions::Table)
                    .col(ClientOnlineSessions::ClientId)
                    .col(ClientOnlineSessions::IpAddress)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_online_sessions_client_ip")
                    .table(ClientOnlineSessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Clients::Table)
                    .drop_column(Clients::MaxDevices)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Clients {
    Table,
    MaxDevices,
}

#[derive(Iden)]
enum ClientOnlineSessions {
    Table,
    ClientId,
    IpAddress,
}

fn integer_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).integer().to_owned()
}

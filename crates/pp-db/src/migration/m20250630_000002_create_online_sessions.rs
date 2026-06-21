use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ClientOnlineSessions::Table)
                    .if_not_exists()
                    .col(pk_uuid(ClientOnlineSessions::Id))
                    .col(uuid(ClientOnlineSessions::ClientId))
                    .col(uuid(ClientOnlineSessions::NodeId))
                    .col(string(ClientOnlineSessions::IpAddress))
                    .col(string_null(ClientOnlineSessions::InboundTag))
                    .col(timestamp(ClientOnlineSessions::ConnectedAt))
                    .col(timestamp(ClientOnlineSessions::LastActiveAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_online_sessions_client")
                    .table(ClientOnlineSessions::Table)
                    .col(ClientOnlineSessions::ClientId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_online_sessions_node")
                    .table(ClientOnlineSessions::Table)
                    .col(ClientOnlineSessions::NodeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_online_sessions_last_active")
                    .table(ClientOnlineSessions::Table)
                    .col(ClientOnlineSessions::LastActiveAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ClientOnlineSessions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum ClientOnlineSessions {
    Table,
    Id,
    ClientId,
    NodeId,
    IpAddress,
    InboundTag,
    ConnectedAt,
    LastActiveAt,
}

fn pk_uuid<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().not_null().primary_key().to_owned()
}
fn uuid<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().not_null().to_owned()
}
fn string<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().not_null().to_owned()
}
fn string_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().to_owned()
}
fn timestamp<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .timestamp_with_time_zone()
        .not_null()
        .extra("DEFAULT CURRENT_TIMESTAMP".to_string())
        .to_owned()
}

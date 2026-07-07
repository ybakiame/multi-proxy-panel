use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InboundHosts::Table)
                    .col(uuid_pk(InboundHosts::Id))
                    .col(uuid_not_null(InboundHosts::ProtocolConfigId))
                    .col(uuid_not_null(InboundHosts::NodeId))
                    .col(string_not_null(InboundHosts::Remark))
                    .col(string_not_null(InboundHosts::Address))
                    .col(integer_not_null(InboundHosts::Port))
                    .col(string_null(InboundHosts::Sni))
                    .col(string_null(InboundHosts::Host))
                    .col(string_null(InboundHosts::Path))
                    .col(string_null(InboundHosts::Security))
                    .col(string_null(InboundHosts::Alpn))
                    .col(string_null(InboundHosts::Fingerprint))
                    .col(boolean_not_null(InboundHosts::IsActive).default(true))
                    .col(timestamp_not_null(InboundHosts::CreatedAt))
                    .col(timestamp_not_null(InboundHosts::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_inbound_hosts_config_node")
                    .table(InboundHosts::Table)
                    .col(InboundHosts::ProtocolConfigId)
                    .col(InboundHosts::NodeId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InboundHosts::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum InboundHosts {
    Table,
    Id,
    ProtocolConfigId,
    NodeId,
    Remark,
    Address,
    Port,
    Sni,
    Host,
    Path,
    Security,
    Alpn,
    Fingerprint,
    IsActive,
    CreatedAt,
    UpdatedAt,
}

fn uuid_pk<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().not_null().primary_key().to_owned()
}

fn uuid_not_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().not_null().to_owned()
}

fn string_not_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().not_null().to_owned()
}

fn string_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().to_owned()
}

fn integer_not_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).integer().not_null().to_owned()
}

fn boolean_not_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).boolean().not_null().to_owned()
}

fn timestamp_not_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .timestamp_with_time_zone()
        .not_null()
        .to_owned()
}

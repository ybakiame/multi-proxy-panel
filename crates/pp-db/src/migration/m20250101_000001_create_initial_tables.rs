use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(pk_uuid(Users::Id))
                    .col(string_uniq(Users::Username))
                    .col(string(Users::PasswordHash))
                    .col(string(Users::Role))
                    .col(string(Users::Status))
                    .col(timestamp(Users::CreatedAt))
                    .col(timestamp(Users::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Nodes::Table)
                    .if_not_exists()
                    .col(pk_uuid(Nodes::Id))
                    .col(string(Nodes::Name))
                    .col(string(Nodes::Hostname))
                    .col(string(Nodes::Address))
                    .col(string(Nodes::TokenHash))
                    .col(json(Nodes::CoresAvailable))
                    .col(json_null(Nodes::Labels))
                    .col(string(Nodes::Status))
                    .col(timestamp_null(Nodes::LastSeenAt))
                    .col(timestamp(Nodes::CreatedAt))
                    .col(timestamp(Nodes::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProtocolConfigs::Table)
                    .if_not_exists()
                    .col(pk_uuid(ProtocolConfigs::Id))
                    .col(string(ProtocolConfigs::Name))
                    .col(string(ProtocolConfigs::ProtocolType))
                    .col(string(ProtocolConfigs::CoreType))
                    .col(integer(ProtocolConfigs::ListenPort))
                    .col(string(ProtocolConfigs::ListenAddress))
                    .col(json(ProtocolConfigs::Settings))
                    .col(json_null(ProtocolConfigs::TlsSettings))
                    .col(timestamp(ProtocolConfigs::CreatedAt))
                    .col(timestamp(ProtocolConfigs::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NodeBindings::Table)
                    .if_not_exists()
                    .col(pk_uuid(NodeBindings::Id))
                    .col(uuid(NodeBindings::NodeId))
                    .col(uuid(NodeBindings::ProtocolConfigId))
                    .col(json_null(NodeBindings::OverrideSettings))
                    .col(boolean(NodeBindings::IsActive))
                    .col(timestamp(NodeBindings::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_bindings_node")
                    .table(NodeBindings::Table)
                    .col(NodeBindings::NodeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_bindings_config")
                    .table(NodeBindings::Table)
                    .col(NodeBindings::ProtocolConfigId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Clients::Table)
                    .if_not_exists()
                    .col(pk_uuid(Clients::Id))
                    .col(uuid(Clients::UserId))
                    .col(string(Clients::Name))
                    .col(string_null(Clients::Email))
                    .col(big_integer(Clients::TrafficLimitBytes))
                    .col(big_integer(Clients::TrafficUsedBytes))
                    .col(timestamp_null(Clients::ExpiryDate))
                    .col(integer_null(Clients::ResetDay))
                    .col(string(Clients::Status))
                    .col(timestamp(Clients::CreatedAt))
                    .col(timestamp(Clients::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SubscriptionTemplates::Table)
                    .if_not_exists()
                    .col(pk_uuid(SubscriptionTemplates::Id))
                    .col(string(SubscriptionTemplates::Name))
                    .col(string(SubscriptionTemplates::Format))
                    .col(json_null(SubscriptionTemplates::BaseConfig))
                    .col(json_null(SubscriptionTemplates::FilterRules))
                    .col(json_null(SubscriptionTemplates::CustomHeaders))
                    .col(timestamp(SubscriptionTemplates::CreatedAt))
                    .col(timestamp(SubscriptionTemplates::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Subscriptions::Table)
                    .if_not_exists()
                    .col(pk_uuid(Subscriptions::Id))
                    .col(uuid(Subscriptions::ClientId))
                    .col(uuid(Subscriptions::TemplateId))
                    .col(string(Subscriptions::Token))
                    .col(string(Subscriptions::UrlPath))
                    .col(timestamp_null(Subscriptions::ExpireAt))
                    .col(boolean(Subscriptions::IsActive))
                    .col(timestamp_null(Subscriptions::LastAccessedAt))
                    .col(timestamp(Subscriptions::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TrafficRecords::Table)
                    .if_not_exists()
                    .col(pk_uuid(TrafficRecords::Id))
                    .col(uuid_null(TrafficRecords::NodeId))
                    .col(uuid_null(TrafficRecords::ProtocolConfigId))
                    .col(uuid_null(TrafficRecords::ClientId))
                    .col(timestamp(TrafficRecords::HourBucket))
                    .col(big_integer(TrafficRecords::UploadBytes))
                    .col(big_integer(TrafficRecords::DownloadBytes))
                    .col(timestamp(TrafficRecords::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_traffic_hour")
                    .table(TrafficRecords::Table)
                    .col(TrafficRecords::HourBucket)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(HostMetrics::Table)
                    .if_not_exists()
                    .col(pk_uuid(HostMetrics::Id))
                    .col(uuid(HostMetrics::NodeId))
                    .col(timestamp(HostMetrics::Timestamp))
                    .col(float(HostMetrics::CpuPercent))
                    .col(big_integer(HostMetrics::MemUsed))
                    .col(big_integer(HostMetrics::MemTotal))
                    .col(big_integer(HostMetrics::DiskUsed))
                    .col(big_integer(HostMetrics::DiskTotal))
                    .col(big_integer(HostMetrics::NetRx))
                    .col(big_integer(HostMetrics::NetTx))
                    .col(float(HostMetrics::LoadAvg1))
                    .col(float(HostMetrics::LoadAvg5))
                    .col(float(HostMetrics::LoadAvg15))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SystemLogs::Table)
                    .if_not_exists()
                    .col(pk_uuid(SystemLogs::Id))
                    .col(string(SystemLogs::Level))
                    .col(string(SystemLogs::Source))
                    .col(text(SystemLogs::Message))
                    .col(json_null(SystemLogs::Metadata))
                    .col(timestamp(SystemLogs::CreatedAt))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(SystemLogs::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(HostMetrics::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(TrafficRecords::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Subscriptions::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(SubscriptionTemplates::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Clients::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(NodeBindings::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(ProtocolConfigs::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Nodes::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Users::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Users {
    Table, Id, Username, PasswordHash, Role, Status, CreatedAt, UpdatedAt,
}

#[derive(Iden)]
enum Nodes {
    Table, Id, Name, Hostname, Address, TokenHash,
    CoresAvailable, Labels, Status, LastSeenAt, CreatedAt, UpdatedAt,
}

#[derive(Iden)]
enum ProtocolConfigs {
    Table, Id, Name, ProtocolType, CoreType, ListenPort, ListenAddress,
    Settings, TlsSettings, CreatedAt, UpdatedAt,
}

#[derive(Iden)]
enum NodeBindings {
    Table, Id, NodeId, ProtocolConfigId, OverrideSettings, IsActive, CreatedAt,
}

#[derive(Iden)]
enum Clients {
    Table, Id, UserId, Name, Email, TrafficLimitBytes, TrafficUsedBytes,
    ExpiryDate, ResetDay, Status, CreatedAt, UpdatedAt,
}

#[derive(Iden)]
enum SubscriptionTemplates {
    Table, Id, Name, Format, BaseConfig, FilterRules, CustomHeaders, CreatedAt, UpdatedAt,
}

#[derive(Iden)]
enum Subscriptions {
    Table, Id, ClientId, TemplateId, Token, UrlPath,
    ExpireAt, IsActive, LastAccessedAt, CreatedAt,
}

#[derive(Iden)]
enum TrafficRecords {
    Table, Id, NodeId, ProtocolConfigId, ClientId,
    HourBucket, UploadBytes, DownloadBytes, CreatedAt,
}

#[derive(Iden)]
enum HostMetrics {
    Table, Id, NodeId, Timestamp, CpuPercent,
    MemUsed, MemTotal, DiskUsed, DiskTotal,
    NetRx, NetTx, LoadAvg1, LoadAvg5, LoadAvg15,
}

#[derive(Iden)]
enum SystemLogs {
    Table, Id, Level, Source, Message, Metadata, CreatedAt,
}

fn pk_uuid<C: Iden + 'static>(c: C) -> ColumnDef {
    // Application-layer UUID generation for cross-backend compatibility.
    // PostgreSQL could use DEFAULT gen_random_uuid(), but SQLite does not support it.
    ColumnDef::new(c).uuid().not_null().primary_key().to_owned()
}
fn uuid<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).uuid().not_null().to_owned() }
fn uuid_null<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).uuid().to_owned() }
fn string<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).string().not_null().to_owned() }
fn string_uniq<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).string().not_null().unique_key().to_owned() }
fn string_null<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).string().to_owned() }
fn text<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).text().not_null().to_owned() }
fn integer<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).integer().not_null().to_owned() }
fn integer_null<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).integer().to_owned() }
fn big_integer<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).big_integer().not_null().to_owned() }
fn boolean<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).boolean().not_null().to_owned() }
fn float<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).float().not_null().to_owned() }
fn timestamp<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).timestamp_with_time_zone().not_null().extra("DEFAULT NOW()".to_string()).to_owned()
}
fn timestamp_null<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).timestamp_with_time_zone().to_owned() }
fn json<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).json().not_null().to_owned() }
fn json_null<C: Iden + 'static>(c: C) -> ColumnDef { ColumnDef::new(c).json().to_owned() }

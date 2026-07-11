use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AgentLogs::Table)
                    .if_not_exists()
                    .col(pk_uuid(AgentLogs::Id))
                    .col(uuid(AgentLogs::NodeId))
                    .col(string(AgentLogs::Level))
                    .col(string(AgentLogs::Target))
                    .col(text(AgentLogs::Message))
                    .col(json_null(AgentLogs::Fields))
                    .col(timestamp(AgentLogs::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_agent_logs_node_id_created_at")
                    .table(AgentLogs::Table)
                    .col(AgentLogs::NodeId)
                    .col(AgentLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentLogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AgentLogs {
    Table,
    Id,
    NodeId,
    Level,
    Target,
    Message,
    Fields,
    CreatedAt,
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

fn text<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).text().not_null().to_owned()
}

fn json_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).json().to_owned()
}

fn timestamp<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .timestamp_with_time_zone()
        .not_null()
        .extra("DEFAULT CURRENT_TIMESTAMP".to_string())
        .to_owned()
}

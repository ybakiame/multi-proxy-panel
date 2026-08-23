use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_agent_logs_node_target_created")
                    .table(AgentLogs::Table)
                    .col(AgentLogs::NodeId)
                    .col(AgentLogs::Target)
                    .col(AgentLogs::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_agent_logs_node_target_created")
                    .table(AgentLogs::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AgentLogs {
    Table,
    NodeId,
    Target,
    CreatedAt,
}

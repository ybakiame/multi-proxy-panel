use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove old node-level group bindings.
        manager
            .drop_table(Table::drop().table(NodeGroupBindings::Table).to_owned())
            .await?;

        // Create binding-level group bindings.
        manager
            .create_table(
                Table::create()
                    .table(NodeBindingGroupBindings::Table)
                    .if_not_exists()
                    .col(pk_uuid(NodeBindingGroupBindings::Id))
                    .col(uuid(NodeBindingGroupBindings::NodeBindingId))
                    .col(uuid(NodeBindingGroupBindings::GroupId))
                    .col(timestamp(NodeBindingGroupBindings::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_binding_group_bindings_binding")
                    .table(NodeBindingGroupBindings::Table)
                    .col(NodeBindingGroupBindings::NodeBindingId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_binding_group_bindings_group")
                    .table(NodeBindingGroupBindings::Table)
                    .col(NodeBindingGroupBindings::GroupId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(NodeBindingGroupBindings::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NodeGroupBindings::Table)
                    .if_not_exists()
                    .col(pk_uuid(NodeGroupBindings::Id))
                    .col(uuid(NodeGroupBindings::NodeId))
                    .col(uuid(NodeGroupBindings::GroupId))
                    .col(timestamp(NodeGroupBindings::CreatedAt))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum NodeGroupBindings {
    Table,
    Id,
    NodeId,
    GroupId,
    CreatedAt,
}

#[derive(Iden)]
enum NodeBindingGroupBindings {
    Table,
    Id,
    NodeBindingId,
    GroupId,
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
        .extra("DEFAULT CURRENT_TIMESTAMP".to_string())
        .to_owned()
}

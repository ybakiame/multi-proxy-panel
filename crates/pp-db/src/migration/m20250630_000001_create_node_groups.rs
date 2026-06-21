use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NodeGroups::Table)
                    .if_not_exists()
                    .col(pk_uuid(NodeGroups::Id))
                    .col(string(NodeGroups::Name))
                    .col(string_null(NodeGroups::Description))
                    .col(json_null(NodeGroups::Labels))
                    .col(timestamp(NodeGroups::CreatedAt))
                    .col(timestamp(NodeGroups::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ClientGroupBindings::Table)
                    .if_not_exists()
                    .col(pk_uuid(ClientGroupBindings::Id))
                    .col(uuid(ClientGroupBindings::ClientId))
                    .col(uuid(ClientGroupBindings::GroupId))
                    .col(timestamp(ClientGroupBindings::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_client_group_bindings_client")
                    .table(ClientGroupBindings::Table)
                    .col(ClientGroupBindings::ClientId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_client_group_bindings_group")
                    .table(ClientGroupBindings::Table)
                    .col(ClientGroupBindings::GroupId)
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

        manager
            .create_index(
                Index::create()
                    .name("idx_node_group_bindings_node")
                    .table(NodeGroupBindings::Table)
                    .col(NodeGroupBindings::NodeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_group_bindings_group")
                    .table(NodeGroupBindings::Table)
                    .col(NodeGroupBindings::GroupId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NodeGroupBindings::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ClientGroupBindings::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NodeGroups::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum NodeGroups {
    Table,
    Id,
    Name,
    Description,
    Labels,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ClientGroupBindings {
    Table,
    Id,
    ClientId,
    GroupId,
    CreatedAt,
}

#[derive(Iden)]
enum NodeGroupBindings {
    Table,
    Id,
    NodeId,
    GroupId,
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
fn string_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().to_owned()
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

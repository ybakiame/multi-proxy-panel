use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Webhooks::Table)
                    .if_not_exists()
                    .col(pk_uuid(Webhooks::Id))
                    .col(string(Webhooks::Name))
                    .col(string(Webhooks::Url))
                    .col(json(Webhooks::Events))
                    .col(string_null(Webhooks::Secret))
                    .col(boolean(Webhooks::IsActive))
                    .col(timestamp(Webhooks::CreatedAt))
                    .col(timestamp(Webhooks::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_webhooks_active")
                    .table(Webhooks::Table)
                    .col(Webhooks::IsActive)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Webhooks::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Webhooks {
    Table,
    Id,
    Name,
    Url,
    Events,
    Secret,
    IsActive,
    CreatedAt,
    UpdatedAt,
}

fn pk_uuid<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).uuid().not_null().primary_key().to_owned()
}
fn string<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().not_null().to_owned()
}
fn string_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).string().to_owned()
}
fn json<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).json().not_null().to_owned()
}
fn boolean<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).boolean().not_null().to_owned()
}
fn timestamp<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .timestamp_with_time_zone()
        .not_null()
        .extra("DEFAULT NOW()".to_string())
        .to_owned()
}

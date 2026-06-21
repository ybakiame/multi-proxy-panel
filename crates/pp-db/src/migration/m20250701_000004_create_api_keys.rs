use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ApiKeys::Table)
                    .if_not_exists()
                    .col(pk_uuid(ApiKeys::Id))
                    .col(string(ApiKeys::Name))
                    .col(string(ApiKeys::KeyHash))
                    .col(json(ApiKeys::Scopes))
                    .col(json_null(ApiKeys::IpAllowlist))
                    .col(integer_null(ApiKeys::RateLimit))
                    .col(timestamp_null(ApiKeys::ExpiresAt))
                    .col(boolean(ApiKeys::IsActive))
                    .col(timestamp(ApiKeys::CreatedAt))
                    .col(timestamp(ApiKeys::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_hash")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::KeyHash)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApiKeys::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum ApiKeys {
    Table,
    Id,
    Name,
    KeyHash,
    Scopes,
    IpAllowlist,
    RateLimit,
    ExpiresAt,
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
fn json<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).json().not_null().to_owned()
}
fn json_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).json().to_owned()
}
fn integer_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).integer().to_owned()
}
fn boolean<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).boolean().not_null().to_owned()
}
fn timestamp_null<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c).timestamp_with_time_zone().to_owned()
}
fn timestamp<C: Iden + 'static>(c: C) -> ColumnDef {
    ColumnDef::new(c)
        .timestamp_with_time_zone()
        .not_null()
        .extra("DEFAULT CURRENT_TIMESTAMP".to_string())
        .to_owned()
}

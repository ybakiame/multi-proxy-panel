use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite does not support modifying foreign key constraints on existing
        // tables. Dropping the column is sufficient because SQLite's
        // DROP COLUMN also removes associated indexes and foreign keys.
        if manager.get_database_backend() != sea_orm::DbBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_subscriptions_template")
                        .table(Subscriptions::Table)
                        .to_owned(),
                )
                .await?;
        }

        // Drop template_id column from subscriptions.
        manager
            .alter_table(
                Table::alter()
                    .table(Subscriptions::Table)
                    .drop_column(Subscriptions::TemplateId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite does not support ADD COLUMN with a foreign key constraint.
        // For SQLite we add the column without the FK; for other backends we
        // also recreate the foreign key.
        manager
            .alter_table(
                Table::alter()
                    .table(Subscriptions::Table)
                    .add_column(ColumnDef::new(Subscriptions::TemplateId).uuid().null())
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != sea_orm::DbBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_subscriptions_template")
                        .from(Subscriptions::Table, Subscriptions::TemplateId)
                        .to(SubscriptionTemplates::Table, SubscriptionTemplates::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}

#[derive(Iden)]
enum Subscriptions {
    Table,
    TemplateId,
}

#[derive(Iden)]
enum SubscriptionTemplates {
    Table,
    Id,
}

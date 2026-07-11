use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite does not support altering column types directly.
        // Recreate the table with base_config as TEXT instead of JSON.
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            CREATE TABLE subscription_templates_new (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                format TEXT NOT NULL,
                base_config TEXT,
                filter_rules TEXT,
                custom_headers TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
            INSERT INTO subscription_templates_new
                (id, name, format, base_config, filter_rules, custom_headers, created_at, updated_at)
            SELECT
                id,
                name,
                format,
                base_config,
                filter_rules,
                custom_headers,
                created_at,
                updated_at
            FROM subscription_templates
            "#,
        )
        .await?;

        db.execute_unprepared("DROP TABLE subscription_templates")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE subscription_templates_new RENAME TO subscription_templates",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reverse: recreate with JSON column type.
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            CREATE TABLE subscription_templates_new (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                format TEXT NOT NULL,
                base_config TEXT,
                filter_rules TEXT,
                custom_headers TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
            INSERT INTO subscription_templates_new
                (id, name, format, base_config, filter_rules, custom_headers, created_at, updated_at)
            SELECT
                id,
                name,
                format,
                base_config,
                filter_rules,
                custom_headers,
                created_at,
                updated_at
            FROM subscription_templates
            "#,
        )
        .await?;

        db.execute_unprepared("DROP TABLE subscription_templates")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE subscription_templates_new RENAME TO subscription_templates",
        )
        .await?;

        Ok(())
    }
}

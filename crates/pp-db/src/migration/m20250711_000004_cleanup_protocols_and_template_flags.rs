use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

const SINGBOX_BUILTIN_TEMPLATE: &str = include_str!("../templates/singbox_builtin.json");
const CLASH_BUILTIN_TEMPLATE: &str = include_str!("../templates/clash_builtin.yaml");

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Clean up unsupported protocol configs and their bindings.
        db.execute_unprepared(
            r#"
            DELETE FROM node_bindings
            WHERE protocol_config_id IN (
                SELECT id FROM protocol_configs
                WHERE protocol_type IN (
                    'vless_vision', 'vmess', 'trojan', 'shadowsocks2022', 'tuic_v5'
                )
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
            DELETE FROM protocol_configs
            WHERE protocol_type IN (
                'vless_vision', 'vmess', 'trojan', 'shadowsocks2022', 'tuic_v5'
            )
            "#,
        )
        .await?;

        // 2. Add builtin / enabled flags to subscription_templates.
        db.execute_unprepared(
            "ALTER TABLE subscription_templates ADD COLUMN is_builtin INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE subscription_templates ADD COLUMN is_enabled INTEGER NOT NULL DEFAULT 1",
        )
        .await?;

        // 3. Mark existing templates as user-defined.
        db.execute_unprepared("UPDATE subscription_templates SET is_builtin = 0")
            .await?;

        // 4. Insert or update builtin templates by format.
        let now = chrono::Utc::now().to_rfc3339();
        for (format, name, content) in [
            ("sing-box", "Builtin SingBox", SINGBOX_BUILTIN_TEMPLATE),
            ("clash", "Builtin Clash", CLASH_BUILTIN_TEMPLATE),
        ] {
            let escaped = content.replace("'", "''");
            let format_escaped = format.replace("'", "''");
            let name_escaped = name.replace("'", "''");

            let existing = db
                .query_one(Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    format!(
                        "SELECT id FROM subscription_templates WHERE is_builtin = 1 AND format = '{}'",
                        format_escaped
                    ),
                ))
                .await?;

            if existing.is_some() {
                db.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    format!(
                        "UPDATE subscription_templates SET name = '{}', base_config = '{}', updated_at = '{}', is_enabled = 1 WHERE is_builtin = 1 AND format = '{}'",
                        name_escaped, escaped, now, format_escaped
                    ),
                ))
                .await?;
            } else {
                let id = uuid::Uuid::new_v4().to_string();
                db.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    format!(
                        "INSERT INTO subscription_templates (id, name, format, base_config, filter_rules, custom_headers, created_at, updated_at, is_builtin, is_enabled) VALUES ('{}', '{}', '{}', '{}', NULL, NULL, '{}', '{}', 1, 1)",
                        id, name_escaped, format_escaped, escaped, now, now
                    ),
                ))
                .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DELETE FROM subscription_templates WHERE is_builtin = 1")
            .await?;

        db.execute_unprepared("ALTER TABLE subscription_templates DROP COLUMN is_builtin")
            .await?;
        db.execute_unprepared("ALTER TABLE subscription_templates DROP COLUMN is_enabled")
            .await?;

        Ok(())
    }
}

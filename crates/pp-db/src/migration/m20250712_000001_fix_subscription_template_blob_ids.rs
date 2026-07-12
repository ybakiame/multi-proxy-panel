use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Some legacy subscription_templates rows were inserted with BLOB ids,
        // while the entity model expects TEXT. Convert them to UUID strings.
        let rows = db
            .query_all(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT id FROM subscription_templates WHERE typeof(id) = 'blob'".to_string(),
            ))
            .await?;

        for row in rows {
            let blob: Vec<u8> = row.try_get_by_index(0)?;
            if blob.len() != 16 {
                continue;
            }
            let bytes: [u8; 16] = blob
                .clone()
                .try_into()
                .map_err(|_| DbErr::Custom("invalid blob id length".into()))?;
            let uuid = uuid::Uuid::from_bytes(bytes);
            db.execute(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                "UPDATE subscription_templates SET id = ? WHERE id = ?",
                [
                    sea_orm::Value::from(uuid.to_string()),
                    sea_orm::Value::Bytes(Some(Box::new(blob))),
                ],
            ))
            .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

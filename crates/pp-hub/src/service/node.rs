use pp_common::PanelResult;
use pp_db::entities::node;
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

#[allow(dead_code)]
pub struct NodeService;

#[allow(dead_code)]
impl NodeService {
    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> PanelResult<Option<node::Model>> {
        Ok(node::Entity::find_by_id(id).one(db).await?)
    }

    pub async fn list_all(db: &DatabaseConnection) -> PanelResult<Vec<node::Model>> {
        Ok(node::Entity::find().all(db).await?)
    }

    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> PanelResult<bool> {
        let res = node::Entity::delete_by_id(id).exec(db).await?;
        Ok(res.rows_affected > 0)
    }
}

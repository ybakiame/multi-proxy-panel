use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "nodes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub address: String,
    pub domain: Option<String>,
    pub token_hash: String,
    pub cores_available: Json,
    pub labels: Option<Json>,
    pub usage_coefficient: f32,
    pub status: String,
    pub parent_id: Option<Uuid>,
    pub last_seen_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::node_binding::Entity")]
    NodeBindings,
    #[sea_orm(has_many = "super::host_metric::Entity")]
    HostMetrics,
    #[sea_orm(has_many = "super::traffic_record::Entity")]
    TrafficRecords,
}

impl Related<super::node_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::NodeBindings.def()
    }
}
impl Related<super::host_metric::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::HostMetrics.def()
    }
}
impl Related<super::traffic_record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TrafficRecords.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

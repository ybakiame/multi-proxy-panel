use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "host_metrics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub node_id: Uuid,
    pub timestamp: DateTimeWithTimeZone,
    pub cpu_percent: f32,
    pub mem_used: i64,
    pub mem_total: i64,
    pub disk_used: i64,
    pub disk_total: i64,
    pub net_rx: i64,
    pub net_tx: i64,
    pub load_avg1: f32,
    pub load_avg5: f32,
    pub load_avg15: f32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::NodeId",
        to = "super::node::Column::Id"
    )]
    Node,
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

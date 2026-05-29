use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "traffic_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub node_id: Option<Uuid>,
    pub protocol_config_id: Option<Uuid>,
    pub client_id: Option<Uuid>,
    pub hour_bucket: DateTimeWithTimeZone,
    pub upload_bytes: i64,
    pub download_bytes: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::NodeId",
        to = "super::node::Column::Id"
    )]
    Node,
    #[sea_orm(
        belongs_to = "super::protocol_config::Entity",
        from = "Column::ProtocolConfigId",
        to = "super::protocol_config::Column::Id"
    )]
    ProtocolConfig,
    #[sea_orm(
        belongs_to = "super::client::Entity",
        from = "Column::ClientId",
        to = "super::client::Column::Id"
    )]
    Client,
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}
impl Related<super::protocol_config::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProtocolConfig.def()
    }
}
impl Related<super::client::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Client.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "inbound_hosts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub protocol_config_id: Uuid,
    pub node_id: Uuid,
    pub remark: String,
    pub address: String,
    pub port: i32,
    pub sni: Option<String>,
    pub host: Option<String>,
    pub path: Option<String>,
    pub security: Option<String>,
    pub alpn: Option<String>,
    pub fingerprint: Option<String>,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::protocol_config::Entity",
        from = "Column::ProtocolConfigId",
        to = "super::protocol_config::Column::Id"
    )]
    ProtocolConfig,
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::NodeId",
        to = "super::node::Column::Id"
    )]
    Node,
}

impl Related<super::protocol_config::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProtocolConfig.def()
    }
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

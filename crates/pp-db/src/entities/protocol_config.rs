use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "protocol_configs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub protocol_type: String,
    pub core_type: String,
    pub listen_port: i32,
    pub listen_address: String,
    pub settings: Json,
    pub tls_settings: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::node_binding::Entity")]
    NodeBindings,
}

impl Related<super::node_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::NodeBindings.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

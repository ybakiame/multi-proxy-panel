use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "node_groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::client_group_binding::Entity")]
    ClientGroupBindings,
    #[sea_orm(has_many = "super::node_binding_group_binding::Entity")]
    NodeBindingGroupBindings,
}

impl Related<super::client_group_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ClientGroupBindings.def()
    }
}

impl Related<super::node_binding_group_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::NodeBindingGroupBindings.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

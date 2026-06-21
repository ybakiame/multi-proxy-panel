use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "node_group_bindings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub node_id: Uuid,
    pub group_id: Uuid,
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
        belongs_to = "super::node_group::Entity",
        from = "Column::GroupId",
        to = "super::node_group::Column::Id"
    )]
    NodeGroup,
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl Related<super::node_group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::NodeGroup.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

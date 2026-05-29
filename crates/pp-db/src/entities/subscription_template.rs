use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "subscription_templates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub format: String,
    pub base_config: Option<Json>,
    pub filter_rules: Option<Json>,
    pub custom_headers: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::subscription::Entity")]
    Subscriptions,
}

impl Related<super::subscription::Entity> for Entity {
    fn to() -> RelationDef { Relation::Subscriptions.def() }
}

impl ActiveModelBehavior for ActiveModel {}

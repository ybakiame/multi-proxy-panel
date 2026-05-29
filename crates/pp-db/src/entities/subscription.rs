use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "subscriptions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub client_id: Uuid,
    pub template_id: Uuid,
    pub token: String,
    pub url_path: String,
    pub expire_at: Option<DateTimeWithTimeZone>,
    pub is_active: bool,
    pub last_accessed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::client::Entity",
        from = "Column::ClientId",
        to = "super::client::Column::Id"
    )]
    Client,
    #[sea_orm(
        belongs_to = "super::subscription_template::Entity",
        from = "Column::TemplateId",
        to = "super::subscription_template::Column::Id"
    )]
    Template,
}

impl Related<super::client::Entity> for Entity {
    fn to() -> RelationDef { Relation::Client.def() }
}
impl Related<super::subscription_template::Entity> for Entity {
    fn to() -> RelationDef { Relation::Template.def() }
}

impl ActiveModelBehavior for ActiveModel {}

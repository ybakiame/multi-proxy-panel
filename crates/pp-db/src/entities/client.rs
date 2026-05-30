use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "clients")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub traffic_limit_bytes: i64,
    pub traffic_used_bytes: i64,
    pub all_time_used_bytes: i64,
    pub expiry_date: Option<DateTimeWithTimeZone>,
    pub reset_day: Option<i32>,
    pub data_limit_reset_strategy: String,
    pub last_traffic_reset_time: Option<DateTimeWithTimeZone>,
    pub max_devices: Option<i32>,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::subscription::Entity")]
    Subscriptions,
    #[sea_orm(has_many = "super::traffic_record::Entity")]
    TrafficRecords,
}

impl Related<super::subscription::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Subscriptions.def()
    }
}
impl Related<super::traffic_record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TrafficRecords.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

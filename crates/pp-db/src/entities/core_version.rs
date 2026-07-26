use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "core_versions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub core_type: String,
    pub version: String,
    /// "release" or "prerelease"
    pub channel: String,
    /// Upstream release publish time; distinguishes builds of rolling tags.
    pub published_at: Option<DateTimeWithTimeZone>,
    /// Upstream target commitish for the release tag, when known.
    pub commit_sha: Option<String>,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

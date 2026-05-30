use chrono::{Datelike, TimeZone, Timelike};
use pp_common::PanelResult;
use pp_db::entities::traffic_record;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

/// In-memory traffic aggregation buffer, flushed to DB hourly.
#[allow(dead_code)]
pub struct TrafficAggregator {
    buffer: RwLock<HashMap<TrafficKey, TrafficValue>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[allow(dead_code)]
struct TrafficKey {
    node_id: Option<Uuid>,
    config_id: Option<Uuid>,
    client_id: Option<Uuid>,
    hour: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct TrafficValue {
    upload: i64,
    download: i64,
}

#[allow(dead_code)]
impl TrafficAggregator {
    pub fn new() -> Self {
        Self {
            buffer: RwLock::new(HashMap::new()),
        }
    }

    pub async fn record(
        &self,
        node_id: Option<Uuid>,
        config_id: Option<Uuid>,
        client_id: Option<Uuid>,
        upload: i64,
        download: i64,
    ) {
        let now = chrono::Utc::now();
        let hour = chrono::Utc
            .with_ymd_and_hms(now.year(), now.month(), now.day(), now.hour(), 0, 0)
            .unwrap();
        let key = TrafficKey {
            node_id,
            config_id,
            client_id,
            hour,
        };

        let mut buf = self.buffer.write().await;
        let entry = buf.entry(key).or_default();
        entry.upload += upload;
        entry.download += download;
    }

    pub async fn flush(&self, db: &DatabaseConnection) -> PanelResult<()> {
        let mut buf = self.buffer.write().await;
        let entries: Vec<_> = buf.drain().collect();
        drop(buf);

        for (key, value) in entries {
            let active = traffic_record::ActiveModel {
                id: Set(Uuid::new_v4()),
                node_id: Set(key.node_id),
                protocol_config_id: Set(key.config_id),
                client_id: Set(key.client_id),
                hour_bucket: Set(key.hour.into()),
                upload_bytes: Set(value.upload),
                download_bytes: Set(value.download),
                created_at: Set(chrono::Utc::now().into()),
            };
            active.insert(db).await?;
        }

        Ok(())
    }
}

impl Default for TrafficAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Query traffic records with optional filters.
#[allow(dead_code)]
pub async fn query_traffic(
    db: &DatabaseConnection,
    node_id: Option<Uuid>,
    client_id: Option<Uuid>,
    start: Option<chrono::DateTime<chrono::Utc>>,
    end: Option<chrono::DateTime<chrono::Utc>>,
) -> PanelResult<Vec<traffic_record::Model>> {
    let mut query = traffic_record::Entity::find();

    if let Some(id) = node_id {
        query = query.filter(traffic_record::Column::NodeId.eq(id));
    }
    if let Some(id) = client_id {
        query = query.filter(traffic_record::Column::ClientId.eq(id));
    }
    if let Some(s) = start {
        query = query.filter(traffic_record::Column::HourBucket.gte(s));
    }
    if let Some(e) = end {
        query = query.filter(traffic_record::Column::HourBucket.lte(e));
    }

    let records = query
        .order_by_desc(traffic_record::Column::HourBucket)
        .all(db)
        .await?;

    Ok(records)
}

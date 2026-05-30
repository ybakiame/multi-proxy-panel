use pp_db::entities::webhook;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{Value, json};

/// Trigger webhooks for a specific event.
pub async fn trigger_event(
    db: &DatabaseConnection,
    event: &str,
    payload: &Value,
) {
    let hooks = match webhook::Entity::find()
        .filter(webhook::Column::IsActive.eq(true))
        .all(db)
        .await
    {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("failed to fetch webhooks: {}", e);
            return;
        }
    };

    for hook in hooks {
        let subscribed: Vec<String> = hook
            .events
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if !subscribed.iter().any(|e| e == event || e == "*") {
            continue;
        }

        let body = json!({
            "event": event,
            "payload": payload,
            "timestamp": chrono::Utc::now().timestamp(),
        });

        let signature = hook.secret.as_ref().map(|secret| {
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
            mac.update(body.to_string().as_bytes());
            let result = mac.finalize();
            format!("sha256={}", hex::encode(result.into_bytes()))
        });

        let url = hook.url.clone();
        let sig = signature.clone();
        let event_owned = event.to_string();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut req = client.post(&url).json(&body);
            if let Some(s) = sig {
                req = req.header("X-Webhook-Signature", s);
            }
            req = req.header("X-Webhook-Event", event_owned);

            match req.send().await {
                Ok(resp) => {
                    tracing::debug!("webhook {} responded with {}", url, resp.status());
                }
                Err(e) => {
                    tracing::warn!("webhook {} failed: {}", url, e);
                }
            }
        });
    }
}

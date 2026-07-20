//! Managed certificates: hub-side catalog and issuance dispatch.
//!
//! Certificates are issued on the node by the agent's built-in ACME client
//! into a unified `<data_dir>/certs/` directory; the hub keeps the catalog
//! (domain, node, status, expiry) and dispatches issuance over gRPC.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use pp_db::entities::{certificate, node};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

pub const CERT_STATUS_PENDING: &str = "pending";
pub const CERT_STATUS_ACTIVE: &str = "active";

fn cert_to_json(c: &certificate::Model, node_name: Option<&str>) -> Value {
    json!({
        "id": c.id,
        "node_id": c.node_id,
        "node_name": node_name,
        "domain": c.domain,
        "status": c.status,
        "challenge_type": c.challenge_type,
        "expires_at": c.expires_at,
        "last_issued_at": c.last_issued_at,
        "last_error": c.last_error,
        "created_at": c.created_at,
    })
}

pub async fn list_certificates(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Value> {
    let mut query = certificate::Entity::find();
    if let Some(node_id) = params.get("node_id").filter(|s| !s.is_empty()) {
        let node_id = Uuid::parse_str(node_id)
            .map_err(|_| ApiError::bad_request("invalid_node_id", "invalid node id"))?;
        query = query.filter(certificate::Column::NodeId.eq(node_id));
    }
    let items = query
        .order_by_asc(certificate::Column::Domain)
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let node_ids: Vec<Uuid> = items.iter().map(|c| c.node_id).collect();
    let nodes: std::collections::HashMap<Uuid, String> = node::Entity::find()
        .filter(node::Column::Id.is_in(node_ids))
        .all(&state.db)
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .map(|n| (n.id, n.name))
        .collect();

    let data: Vec<Value> = items
        .iter()
        .map(|c| cert_to_json(c, nodes.get(&c.node_id).map(|s| s.as_str())))
        .collect();
    Ok(ApiResponse::new(json!({ "certificates": data })))
}

/// Dispatch an issuance request to the node's agent. Offline agents leave
/// the record pending; issuance is re-dispatched when the agent registers.
pub async fn push_issue_request(
    state: &AppState,
    node_id: Uuid,
    cert_id: Uuid,
    domain: &str,
    challenge_type: &str,
) -> anyhow::Result<()> {
    let message = pp_proto::HubMessage {
        payload: Some(pp_proto::hub_message::Payload::CertIssue(
            pp_proto::CertIssueRequest {
                cert_id: cert_id.to_string(),
                domain: domain.to_string(),
                challenge_type: challenge_type.to_string(),
            },
        )),
    };
    state.send_to_agent(node_id, message).await
}

async fn dispatch_issuance(state: &AppState, cert: &certificate::Model) {
    if let Err(e) = push_issue_request(
        state,
        cert.node_id,
        cert.id,
        &cert.domain,
        &cert.challenge_type,
    )
    .await
    {
        tracing::info!(
            "cert {} ({}) stays pending, agent unreachable: {}",
            cert.id,
            cert.domain,
            e
        );
    }
}

#[derive(serde::Deserialize)]
pub struct CreateCertificatePayload {
    pub domain: String,
    pub node_id: Uuid,
    pub challenge_type: Option<String>,
}

pub async fn create_certificate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateCertificatePayload>,
) -> ApiResult<Value> {
    let domain = payload.domain.trim().to_string();
    if domain.is_empty() || domain.contains(' ') {
        return Err(ApiError::bad_request(
            "invalid_domain",
            "domain is required",
        ));
    }
    let challenge_type = match payload.challenge_type.as_deref().unwrap_or("http-01") {
        "http-01" => "http-01".to_string(),
        other => {
            return Err(ApiError::bad_request(
                "invalid_challenge_type",
                format!("unsupported challenge type: {}", other),
            ));
        }
    };

    let node_exists = node::Entity::find_by_id(payload.node_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .is_some();
    if !node_exists {
        return Err(ApiError::not_found("node not found"));
    }

    let existing = certificate::Entity::find()
        .filter(certificate::Column::NodeId.eq(payload.node_id))
        .filter(certificate::Column::Domain.eq(&domain))
        .one(&state.db)
        .await
        .map_err(ApiError::from)?;

    let cert = if let Some(existing) = existing {
        let mut active: certificate::ActiveModel = existing.into();
        active.status = Set(CERT_STATUS_PENDING.to_string());
        active.challenge_type = Set(challenge_type);
        active.last_error = Set(None);
        active.updated_at = Set(chrono::Utc::now().into());
        active.update(&state.db).await.map_err(ApiError::from)?
    } else {
        certificate::ActiveModel {
            id: Set(Uuid::new_v4()),
            node_id: Set(payload.node_id),
            domain: Set(domain),
            status: Set(CERT_STATUS_PENDING.to_string()),
            challenge_type: Set(challenge_type),
            expires_at: Set(None),
            last_issued_at: Set(None),
            last_error: Set(None),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(&state.db)
        .await
        .map_err(ApiError::from)?
    };

    dispatch_issuance(&state, &cert).await;

    let node_name = node::Entity::find_by_id(cert.node_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .map(|n| n.name);
    Ok(ApiResponse::new(cert_to_json(&cert, node_name.as_deref())))
}

pub async fn renew_certificate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let cert = certificate::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("certificate not found"))?;

    let mut active: certificate::ActiveModel = cert.into();
    active.status = Set(CERT_STATUS_PENDING.to_string());
    active.last_error = Set(None);
    active.updated_at = Set(chrono::Utc::now().into());
    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    dispatch_issuance(&state, &updated).await;
    Ok(ApiResponse::new(cert_to_json(&updated, None)))
}

pub async fn delete_certificate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = certificate::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("certificate not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

/// Apply a status report from the agent to the catalog row.
pub async fn apply_cert_status(
    state: &AppState,
    report: pp_proto::CertStatusReport,
) -> Result<(), sea_orm::DbErr> {
    let Ok(id) = Uuid::parse_str(&report.cert_id) else {
        tracing::warn!("ignoring cert status with invalid id {}", report.cert_id);
        return Ok(());
    };
    let Some(cert) = certificate::Entity::find_by_id(id).one(&state.db).await? else {
        tracing::warn!("ignoring cert status for unknown cert {}", id);
        return Ok(());
    };

    let mut active: certificate::ActiveModel = cert.into();
    active.status = Set(report.status.clone());
    active.updated_at = Set(chrono::Utc::now().into());
    if report.status == CERT_STATUS_ACTIVE {
        active.expires_at = Set(if report.expires_at > 0 {
            chrono::DateTime::from_timestamp(report.expires_at, 0).map(|t| t.into())
        } else {
            None
        });
        active.last_issued_at = Set(Some(chrono::Utc::now().into()));
        active.last_error = Set(None);
    } else {
        active.last_error = Set(Some(report.error).filter(|s| !s.is_empty()));
    }
    active.update(&state.db).await?;
    Ok(())
}

/// Re-dispatch issuance for all pending certs of a node (called after the
/// agent registers, so offline-created certs get issued on connect).
pub async fn dispatch_pending_for_node(
    state: &AppState,
    node_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    let pending = certificate::Entity::find()
        .filter(certificate::Column::NodeId.eq(node_id))
        .filter(certificate::Column::Status.eq(CERT_STATUS_PENDING))
        .all(&state.db)
        .await?;
    for cert in pending {
        dispatch_issuance(state, &cert).await;
    }
    Ok(())
}

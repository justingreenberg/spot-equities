use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::db::{models::{KycRecord, Request}, queries};
use std::sync::Arc;
use crate::dinari::client::DinariClient;

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct ListRequestsQuery {
    pub request_type: Option<String>,
    pub status: Option<String>,
    pub requester: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RequestResponse {
    pub requests: Vec<Request>,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_mints: i64,
    pub total_redeems: i64,
    pub pending_mints: i64,
    pub pending_redeems: i64,
}

#[derive(Debug, Serialize)]
pub struct TreasuryResponse {
    pub dinari_usdc_balance: String,
    pub dinari_dshares_held: String,
    pub synthetic_outstanding: String,
    pub backing_ratio: String,
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// GET /api/health
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /api/requests
pub async fn list_requests(
    State(state): State<AppState>,
    Query(params): Query<ListRequestsQuery>,
) -> Result<Json<RequestResponse>, StatusCode> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let requests = queries::list_requests(
        &state.pool,
        params.request_type.as_deref(),
        params.status.as_deref(),
        params.requester.as_deref(),
        limit,
        offset,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RequestResponse { requests }))
}

/// GET /api/requests/:id
pub async fn get_request(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Request>, StatusCode> {
    let request = queries::get_request_by_request_id(&state.pool, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(request))
}

/// GET /api/treasury
pub async fn get_treasury(
    State(state): State<AppState>,
) -> Result<Json<TreasuryResponse>, StatusCode> {
    let snapshot = queries::get_latest_treasury_snapshot(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match snapshot {
        Some(s) => Ok(Json(TreasuryResponse {
            dinari_usdc_balance: s.dinari_usdc_balance,
            dinari_dshares_held: s.dinari_dshares_held,
            synthetic_outstanding: s.synthetic_outstanding,
            backing_ratio: s.backing_ratio,
            last_updated: Some(s.created_at.to_rfc3339()),
        })),
        None => Ok(Json(TreasuryResponse {
            dinari_usdc_balance: "0".to_string(),
            dinari_dshares_held: "0".to_string(),
            synthetic_outstanding: "0".to_string(),
            backing_ratio: "1.0".to_string(),
            last_updated: None,
        })),
    }
}

/// GET /api/stats
pub async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<StatsResponse>, StatusCode> {
    // Simple counts from the database
    let all_requests = queries::list_requests(&state.pool, None, None, None, 10000, 0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_mints = all_requests.iter().filter(|r| r.request_type == "mint").count() as i64;
    let total_redeems = all_requests.iter().filter(|r| r.request_type == "redeem").count() as i64;
    let pending_mints = all_requests
        .iter()
        .filter(|r| r.request_type == "mint" && r.status != "fulfilled" && r.status != "failed")
        .count() as i64;
    let pending_redeems = all_requests
        .iter()
        .filter(|r| r.request_type == "redeem" && r.status != "fulfilled" && r.status != "failed")
        .count() as i64;

    Ok(Json(StatsResponse {
        total_mints,
        total_redeems,
        pending_mints,
        pending_redeems,
    }))
}

// ── KYC Endpoints ──

#[derive(Debug, Deserialize)]
pub struct InitKycRequest {
    pub wallet_address: String,
    pub redirect_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KycSessionResponse {
    pub wallet_address: String,
    pub kyc_url: String,
    pub status: String,
    pub dinari_account_id: String,
}

#[derive(Debug, Serialize)]
pub struct KycStatusResponse {
    pub wallet_address: String,
    pub status: String,
    pub role_granted: bool,
    pub kyc_url: Option<String>,
    pub dinari_account_id: Option<String>,
    pub rejected_reason: Option<String>,
    pub approved_at: Option<String>,
    pub role_granted_at: Option<String>,
    pub role_tx_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListKycQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminKycListResponse {
    pub records: Vec<KycRecord>,
}

#[derive(Debug, Deserialize)]
pub struct MarkRoleGrantedRequest {
    pub wallet_address: String,
    pub tx_hash: String,
}

/// POST /api/kyc/init — Initiate KYC for a wallet address.
/// Creates a Dinari managed KYC session and returns the redirect URL.
pub async fn init_kyc(
    State(state): State<AppState>,
    Json(body): Json<InitKycRequest>,
) -> Result<Json<KycSessionResponse>, StatusCode> {
    // Check if already has a KYC record
    let existing = queries::get_kyc_record(&state.pool, &body.wallet_address)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(record) = existing {
        if record.status == "approved" {
            // Already approved, return existing info
            return Ok(Json(KycSessionResponse {
                wallet_address: record.wallet_address,
                kyc_url: record.kyc_url.unwrap_or_default(),
                status: record.status,
                dinari_account_id: record.dinari_account_id.unwrap_or_default(),
            }));
        }
        if let Some(url) = &record.kyc_url {
            // Session already exists, return the URL
            return Ok(Json(KycSessionResponse {
                wallet_address: record.wallet_address,
                kyc_url: url.clone(),
                status: record.status,
                dinari_account_id: record.dinari_account_id.unwrap_or_default(),
            }));
        }
    }

    // Create new Dinari KYC session
    let session = state
        .dinari
        .create_kyc_session(&body.wallet_address, body.redirect_url.as_deref())
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Save to database
    queries::upsert_kyc_record(
        &state.pool,
        &body.wallet_address,
        Some(&session.id),
        Some(&session.kyc_url),
        &session.status.to_string(),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(KycSessionResponse {
        wallet_address: body.wallet_address,
        kyc_url: session.kyc_url,
        status: session.status.to_string(),
        dinari_account_id: session.id,
    }))
}

/// GET /api/kyc/:wallet_address — Get KYC status for a wallet.
/// Also refreshes the status from Dinari if a session exists.
pub async fn get_kyc_status(
    State(state): State<AppState>,
    Path(wallet_address): Path<String>,
) -> Result<Json<KycStatusResponse>, StatusCode> {
    let record = queries::get_kyc_record(&state.pool, &wallet_address)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match record {
        Some(record) => {
            // If pending/in_review, refresh from Dinari
            if (record.status == "pending" || record.status == "in_review")
                && record.dinari_account_id.is_some()
            {
                let account_id = record.dinari_account_id.as_ref().unwrap();
                if let Ok(dinari_status) = state.dinari.get_kyc_status(account_id).await {
                    let new_status = dinari_status.status.to_string();
                    if new_status != record.status {
                        let approved_at = if new_status == "approved" {
                            Some(chrono::Utc::now().to_rfc3339())
                        } else {
                            None
                        };
                        let _ = queries::update_kyc_status(
                            &state.pool,
                            &wallet_address,
                            &new_status,
                            None,
                            approved_at.as_deref(),
                            dinari_status.rejected_reason.as_deref(),
                        )
                        .await;

                        return Ok(Json(KycStatusResponse {
                            wallet_address,
                            status: new_status,
                            role_granted: record.role_granted,
                            kyc_url: record.kyc_url,
                            dinari_account_id: record.dinari_account_id,
                            rejected_reason: dinari_status.rejected_reason,
                            approved_at,
                            role_granted_at: record.role_granted_at.map(|t| t.to_rfc3339()),
                            role_tx_hash: record.role_tx_hash,
                        }));
                    }
                }
            }

            Ok(Json(KycStatusResponse {
                wallet_address: record.wallet_address,
                status: record.status,
                role_granted: record.role_granted,
                kyc_url: record.kyc_url,
                dinari_account_id: record.dinari_account_id,
                rejected_reason: record.rejected_reason,
                approved_at: record.approved_at.map(|t| t.to_rfc3339()),
                role_granted_at: record.role_granted_at.map(|t| t.to_rfc3339()),
                role_tx_hash: record.role_tx_hash,
            }))
        }
        None => Ok(Json(KycStatusResponse {
            wallet_address,
            status: "not_started".to_string(),
            role_granted: false,
            kyc_url: None,
            dinari_account_id: None,
            rejected_reason: None,
            approved_at: None,
            role_granted_at: None,
            role_tx_hash: None,
        })),
    }
}

/// GET /api/admin/kyc — List all KYC records (admin/manager view).
pub async fn list_kyc(
    State(state): State<AppState>,
    Query(params): Query<ListKycQuery>,
) -> Result<Json<AdminKycListResponse>, StatusCode> {
    let records = queries::list_kyc_records(&state.pool, params.status.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AdminKycListResponse { records }))
}

/// POST /api/admin/kyc/grant-role — Record that MARKET_MAKER_ROLE was granted on-chain.
/// Called after admin runs the grant script and has the tx hash.
pub async fn mark_role_granted(
    State(state): State<AppState>,
    Json(body): Json<MarkRoleGrantedRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Verify the wallet has approved KYC
    let record = queries::get_kyc_record(&state.pool, &body.wallet_address)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if record.status != "approved" {
        return Err(StatusCode::BAD_REQUEST);
    }

    queries::mark_role_granted(&state.pool, &body.wallet_address, &body.tx_hash)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "wallet_address": body.wallet_address,
        "role_granted": true,
        "tx_hash": body.tx_hash
    })))
}

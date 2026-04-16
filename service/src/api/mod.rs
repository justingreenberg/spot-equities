pub mod routes;
pub mod sse;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::dinari::client::DinariClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub dinari: Arc<DinariClient>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health
        .route("/api/health", get(routes::health))
        // Requests
        .route("/api/requests", get(routes::list_requests))
        .route("/api/requests/{id}", get(routes::get_request))
        // Treasury
        .route("/api/treasury", get(routes::get_treasury))
        .route("/api/stats", get(routes::get_stats))
        // KYC
        .route("/api/kyc/init", post(routes::init_kyc))
        .route("/api/kyc/{wallet_address}", get(routes::get_kyc_status))
        // Admin
        .route("/api/admin/kyc", get(routes::list_kyc))
        .route("/api/admin/kyc/grant-role", post(routes::mark_role_granted))
        // SSE
        .route("/api/events", get(sse::events_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

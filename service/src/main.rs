mod api;
mod config;
mod db;
mod dinari;
mod engine;
mod listener;
mod treasury;

use std::net::SocketAddr;
use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::RootProvider;
use tracing::{error, info};

use crate::api::AppState;
use crate::dinari::client::DinariClient;
use crate::engine::fulfiller::Fulfiller;
use crate::engine::settlement::SettlementEngine;
use crate::listener::EventListener;
use crate::treasury::TreasuryReconciler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "spot_equities_service=info,tower_http=info".into()),
        )
        .init();

    // Load config
    let config = config::Config::from_env()?;
    info!("Starting spot-equities service on port {}", config.port);

    // Initialize database
    let pool = db::init_pool(&config.database_url).await?;
    info!("Database initialized");

    // Initialize Dinari clients (one shared for API routes, one for settlement, one for treasury)
    let dinari_for_api = Arc::new(DinariClient::new(
        &config.dinari_api_url,
        &config.dinari_api_key_id,
        &config.dinari_api_secret,
    ));

    // Initialize HyperEVM provider
    let provider = RootProvider::new_http(config.hyperevm_rpc_url.parse()?);

    let vault_address: Address = config.vault_contract_address.parse()?;

    // Initialize components
    let listener = EventListener::new(provider, vault_address, pool.clone());
    let fulfiller = Fulfiller::new(&config.hyperevm_rpc_url, vault_address, &config.operator_private_key)?;
    info!("Operator wallet initialized");

    let settlement_engine = SettlementEngine::new(
        pool.clone(),
        listener,
        DinariClient::new(
            &config.dinari_api_url,
            &config.dinari_api_key_id,
            &config.dinari_api_secret,
        ),
        fulfiller,
        config.ticker.clone(),
    );

    let treasury_reconciler = TreasuryReconciler::new(
        pool.clone(),
        DinariClient::new(&config.dinari_api_url, &config.dinari_api_key_id, &config.dinari_api_secret),
        config.ticker.clone(),
    );

    // Start settlement engine loop
    let settlement_interval = config.settlement_interval_ms;
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(settlement_interval));
        loop {
            interval.tick().await;
            if let Err(e) = settlement_engine.tick().await {
                error!("Settlement engine error: {}", e);
            }
        }
    });

    // Start treasury reconciler loop (every 5 minutes)
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = treasury_reconciler.check().await {
                error!("Treasury reconciler error: {}", e);
            }
        }
    });

    // Start HTTP server
    let state = AppState { pool, dinari: dinari_for_api };
    let app = api::create_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    info!("HTTP server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

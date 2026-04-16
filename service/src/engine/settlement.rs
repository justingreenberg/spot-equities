use sqlx::SqlitePool;
use tracing::{error, info, warn};

use crate::db::queries;
use crate::dinari::client::DinariClient;
use crate::dinari::types::DinariOrderStatus;
use crate::listener::EventListener;

use super::fulfiller::Fulfiller;

/// The main orchestration engine that processes mint/redeem requests.
pub struct SettlementEngine {
    pool: SqlitePool,
    listener: EventListener,
    dinari: DinariClient,
    fulfiller: Fulfiller,
    ticker: String,
}

impl SettlementEngine {
    pub fn new(
        pool: SqlitePool,
        listener: EventListener,
        dinari: DinariClient,
        fulfiller: Fulfiller,
        ticker: String,
    ) -> Self {
        Self {
            pool,
            listener,
            dinari,
            fulfiller,
            ticker,
        }
    }

    /// Run one tick of the settlement loop.
    pub async fn tick(&self) -> anyhow::Result<()> {
        // 1. Poll for new on-chain events
        match self.listener.poll().await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "Detected new on-chain events");
                }
            }
            Err(e) => warn!("Event polling error: {}", e),
        }

        // 2. Promote detected → pending
        self.promote_detected().await;

        // 3. Process pending requests (place Dinari orders)
        self.process_pending().await;

        // 4. Check processing requests (poll Dinari status)
        self.check_processing().await;

        // 5. Prepare completed requests for fulfillment
        self.prepare_fulfillment().await;

        // 6. Fulfill ready requests (submit on-chain txs)
        self.fulfill_ready().await;

        Ok(())
    }

    /// Move detected requests to pending (validation step)
    async fn promote_detected(&self) {
        let requests = match queries::get_requests_by_status(&self.pool, "detected").await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch detected requests: {}", e);
                return;
            }
        };

        for req in requests {
            info!(request_id = req.request_id, "Promoting request to pending");
            if let Err(e) = queries::update_request_status(
                &self.pool,
                req.request_id,
                "pending",
                None, None, None, None, None, None,
            )
            .await
            {
                warn!(request_id = req.request_id, "Failed to promote: {}", e);
            }
        }
    }

    /// Place Dinari orders for pending requests
    async fn process_pending(&self) {
        let requests = match queries::get_requests_by_status(&self.pool, "pending").await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch pending requests: {}", e);
                return;
            }
        };

        for req in requests {
            let idempotency_key = format!("req-{}-{}", req.request_id, req.retry_count);

            let result = if req.request_type == "mint" {
                self.dinari
                    .create_buy_order(&self.ticker, &req.collateral_amount, &idempotency_key)
                    .await
            } else {
                let shares = req.synthetic_amount.as_deref().unwrap_or("0");
                self.dinari
                    .create_sell_order(&self.ticker, shares, &idempotency_key)
                    .await
            };

            match result {
                Ok(order) => {
                    info!(
                        request_id = req.request_id,
                        dinari_order_id = %order.id,
                        "Dinari order placed"
                    );
                    let _ = queries::update_request_status(
                        &self.pool,
                        req.request_id,
                        "processing",
                        Some(&order.id),
                        Some("pending"),
                        None, None, None, None,
                    )
                    .await;
                    // TODO: call markMintProcessing / markRedeemProcessing on-chain
                }
                Err(e) => {
                    error!(request_id = req.request_id, "Dinari order failed: {}", e);
                    let _ = queries::increment_retry(&self.pool, req.request_id, &e.to_string()).await;

                    // After too many retries, mark as failed
                    if req.retry_count >= 5 {
                        warn!(request_id = req.request_id, "Max retries exceeded, marking failed");
                        let _ = queries::update_request_status(
                            &self.pool,
                            req.request_id,
                            "failed",
                            None, None, None, None, None,
                            Some(&e.to_string()),
                        )
                        .await;
                        // TODO: call failMint / failRedeem on-chain
                    }
                }
            }
        }
    }

    /// Poll Dinari for status updates on processing requests
    async fn check_processing(&self) {
        let requests = match queries::get_requests_by_status(&self.pool, "processing").await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch processing requests: {}", e);
                return;
            }
        };

        for req in requests {
            let order_id = match &req.dinari_order_id {
                Some(id) => id.clone(),
                None => {
                    warn!(request_id = req.request_id, "Processing request missing Dinari order ID");
                    continue;
                }
            };

            match self.dinari.get_order(&order_id).await {
                Ok(order) => match order.status {
                    DinariOrderStatus::Completed => {
                        info!(
                            request_id = req.request_id,
                            fill_price = ?order.average_price,
                            fill_shares = ?order.filled_shares,
                            "Dinari order completed"
                        );
                        let _ = queries::update_request_status(
                            &self.pool,
                            req.request_id,
                            "dinari_completed",
                            None,
                            Some("completed"),
                            order.average_price.as_deref(),
                            order.filled_shares.as_deref(),
                            None, None,
                        )
                        .await;
                    }
                    DinariOrderStatus::Failed | DinariOrderStatus::Cancelled => {
                        warn!(
                            request_id = req.request_id,
                            status = ?order.status,
                            "Dinari order failed/cancelled"
                        );
                        let _ = queries::update_request_status(
                            &self.pool,
                            req.request_id,
                            "failed",
                            None,
                            Some(&format!("{:?}", order.status)),
                            None, None, None,
                            Some("Dinari order failed"),
                        )
                        .await;
                        // TODO: call failMint / failRedeem on-chain
                    }
                    _ => {
                        // Still processing, no-op
                    }
                },
                Err(e) => {
                    warn!(
                        request_id = req.request_id,
                        "Failed to poll Dinari order: {}",
                        e
                    );
                }
            }
        }
    }

    /// Calculate fulfillment amounts for completed Dinari orders
    async fn prepare_fulfillment(&self) {
        let requests = match queries::get_requests_by_status(&self.pool, "dinari_completed").await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch dinari_completed requests: {}", e);
                return;
            }
        };

        for req in requests {
            // For mint: calculate synthetic tokens to mint based on fill price
            // For redeem: calculate USDC to return based on sale amount
            // This is where the price → amount conversion happens
            info!(
                request_id = req.request_id,
                request_type = %req.request_type,
                "Preparing fulfillment"
            );

            let _ = queries::update_request_status(
                &self.pool,
                req.request_id,
                "ready_to_fulfill",
                None, None, None, None, None, None,
            )
            .await;
        }
    }

    /// Submit on-chain fulfillment transactions
    async fn fulfill_ready(&self) {
        let requests = match queries::get_requests_by_status(&self.pool, "ready_to_fulfill").await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch ready_to_fulfill requests: {}", e);
                return;
            }
        };

        for req in requests {
            info!(
                request_id = req.request_id,
                request_type = %req.request_type,
                "Submitting on-chain fulfillment"
            );

            // TODO: Submit actual on-chain transaction using the fulfiller
            // For now, we encode the calldata (actual tx submission requires a signer)
            // This will be wired up when we have a wallet provider

            // Placeholder: mark as fulfilled (in production, only after tx confirms)
            let _ = queries::update_request_status(
                &self.pool,
                req.request_id,
                "fulfilled",
                None, None, None, None,
                Some("pending-implementation"),
                None,
            )
            .await;
        }
    }
}

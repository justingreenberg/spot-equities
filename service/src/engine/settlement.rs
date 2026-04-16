use alloy::primitives::U256;
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

        // 3. Process pending requests (place Dinari orders + mark on-chain)
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

    /// Place Dinari orders for pending requests and mark them as processing on-chain
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
                        "Dinari order placed, marking processing on-chain"
                    );

                    // Mark processing on-chain
                    let tx_result = if req.request_type == "mint" {
                        self.fulfiller
                            .mark_mint_processing(req.request_id as u64, &order.id)
                            .await
                    } else {
                        self.fulfiller
                            .mark_redeem_processing(req.request_id as u64, &order.id)
                            .await
                    };

                    match tx_result {
                        Ok(tx_hash) => {
                            info!(request_id = req.request_id, tx_hash, "Marked processing on-chain");
                            let _ = queries::update_request_status(
                                &self.pool,
                                req.request_id,
                                "processing",
                                Some(&order.id),
                                Some("pending"),
                                None, None,
                                Some(&tx_hash),
                                None,
                            )
                            .await;
                        }
                        Err(e) => {
                            // Dinari order was placed but on-chain marking failed.
                            // Still move to processing — the on-chain state will be
                            // reconciled on the next attempt or manually.
                            warn!(
                                request_id = req.request_id,
                                "Failed to mark processing on-chain: {}", e
                            );
                            let _ = queries::update_request_status(
                                &self.pool,
                                req.request_id,
                                "processing",
                                Some(&order.id),
                                Some("pending"),
                                None, None, None,
                                Some(&format!("on-chain mark failed: {}", e)),
                            )
                            .await;
                        }
                    }
                }
                Err(e) => {
                    error!(request_id = req.request_id, "Dinari order failed: {}", e);
                    let _ = queries::increment_retry(&self.pool, req.request_id, &e.to_string()).await;

                    if req.retry_count >= 5 {
                        warn!(request_id = req.request_id, "Max retries exceeded, failing on-chain");

                        let fail_result = if req.request_type == "mint" {
                            self.fulfiller.fail_mint(req.request_id as u64).await
                        } else {
                            self.fulfiller.fail_redeem(req.request_id as u64).await
                        };

                        let tx_hash = match fail_result {
                            Ok(hash) => Some(hash),
                            Err(e2) => {
                                error!(request_id = req.request_id, "Failed to call failRequest on-chain: {}", e2);
                                None
                            }
                        };

                        let _ = queries::update_request_status(
                            &self.pool,
                            req.request_id,
                            "failed",
                            None, None, None, None,
                            tx_hash.as_deref(),
                            Some(&e.to_string()),
                        )
                        .await;
                    }
                }
            }
        }
    }

    /// Poll Dinari for status updates on processing requests.
    /// On failure, call failMint/failRedeem on-chain to refund the MM.
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
                            "Dinari order failed/cancelled, refunding on-chain"
                        );

                        let fail_result = if req.request_type == "mint" {
                            self.fulfiller.fail_mint(req.request_id as u64).await
                        } else {
                            self.fulfiller.fail_redeem(req.request_id as u64).await
                        };

                        let tx_hash = match fail_result {
                            Ok(hash) => {
                                info!(request_id = req.request_id, tx_hash = %hash, "Refund tx confirmed");
                                Some(hash)
                            }
                            Err(e) => {
                                error!(request_id = req.request_id, "Failed to refund on-chain: {}", e);
                                None
                            }
                        };

                        let _ = queries::update_request_status(
                            &self.pool,
                            req.request_id,
                            "failed",
                            None,
                            Some(&format!("{:?}", order.status)),
                            None, None,
                            tx_hash.as_deref(),
                            Some("Dinari order failed"),
                        )
                        .await;
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

    /// Calculate fulfillment amounts for completed Dinari orders.
    /// For mints: synthetic_amount = fill_shares (Dinari shares ≈ synthetic tokens, scaled to 18 dec)
    /// For redeems: collateral_amount = filled_amount (USDC received from sale)
    async fn prepare_fulfillment(&self) {
        let requests = match queries::get_requests_by_status(&self.pool, "dinari_completed").await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch dinari_completed requests: {}", e);
                return;
            }
        };

        for req in requests {
            info!(
                request_id = req.request_id,
                request_type = %req.request_type,
                fill_price = ?req.dinari_fill_price,
                fill_shares = ?req.dinari_fill_shares,
                "Preparing fulfillment"
            );

            // Amounts are calculated here and stored; the next step reads them back.
            // For now, pass through to ready_to_fulfill — the actual U256 conversion
            // happens in fulfill_ready() from the stored fill data.
            let _ = queries::update_request_status(
                &self.pool,
                req.request_id,
                "ready_to_fulfill",
                None, None, None, None, None, None,
            )
            .await;
        }
    }

    /// Submit on-chain fulfillment transactions.
    async fn fulfill_ready(&self) {
        let requests = match queries::get_requests_by_status(&self.pool, "ready_to_fulfill").await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch ready_to_fulfill requests: {}", e);
                return;
            }
        };

        for req in requests {
            let result = if req.request_type == "mint" {
                self.fulfill_mint_request(&req).await
            } else {
                self.fulfill_redeem_request(&req).await
            };

            match result {
                Ok(tx_hash) => {
                    info!(
                        request_id = req.request_id,
                        tx_hash,
                        "Fulfillment confirmed"
                    );
                    let _ = queries::update_request_status(
                        &self.pool,
                        req.request_id,
                        "fulfilled",
                        None, None, None, None,
                        Some(&tx_hash),
                        None,
                    )
                    .await;
                }
                Err(e) => {
                    error!(
                        request_id = req.request_id,
                        "Fulfillment tx failed: {}", e
                    );
                    let _ = queries::update_request_status(
                        &self.pool,
                        req.request_id,
                        "fulfillment_failed",
                        None, None, None, None, None,
                        Some(&e.to_string()),
                    )
                    .await;
                    let _ = queries::increment_retry(&self.pool, req.request_id, &e.to_string()).await;
                }
            }
        }
    }

    /// Fulfill a mint: calculate synthetic tokens from Dinari fill, call fulfillMint on-chain.
    async fn fulfill_mint_request(&self, req: &crate::db::models::Request) -> anyhow::Result<String> {
        // fill_shares from Dinari = number of dShares purchased.
        // Each dShare maps to 1 synthetic token (1e18 wei).
        let fill_shares_str = req.dinari_fill_shares.as_deref().unwrap_or("0");
        let fill_shares: f64 = fill_shares_str.parse().unwrap_or(0.0);

        if fill_shares <= 0.0 {
            anyhow::bail!("Invalid fill shares: {}", fill_shares_str);
        }

        // Convert to 18-decimal U256: shares * 1e18
        let synthetic_wei = (fill_shares * 1e18) as u128;
        let synthetic_amount = U256::from(synthetic_wei);

        info!(
            request_id = req.request_id,
            fill_shares,
            synthetic_amount = %synthetic_amount,
            "Calling fulfillMint"
        );

        self.fulfiller
            .fulfill_mint(req.request_id as u64, synthetic_amount)
            .await
    }

    /// Fulfill a redeem: calculate USDC from Dinari sale, call fulfillRedeem on-chain.
    async fn fulfill_redeem_request(&self, req: &crate::db::models::Request) -> anyhow::Result<String> {
        // filled_amount from Dinari = USDC received from dShare sale.
        // USDC has 6 decimals on HyperEVM.
        let fill_price_str = req.dinari_fill_price.as_deref().unwrap_or("0");
        let fill_shares_str = req.dinari_fill_shares.as_deref().unwrap_or("0");
        let fill_price: f64 = fill_price_str.parse().unwrap_or(0.0);
        let fill_shares: f64 = fill_shares_str.parse().unwrap_or(0.0);

        let usdc_amount = fill_price * fill_shares;
        if usdc_amount <= 0.0 {
            anyhow::bail!("Invalid redemption amount: price={} shares={}", fill_price, fill_shares);
        }

        // Convert to 6-decimal U256: amount * 1e6
        let collateral_wei = (usdc_amount * 1e6) as u128;
        let collateral_amount = U256::from(collateral_wei);

        info!(
            request_id = req.request_id,
            usdc_amount,
            collateral_amount = %collateral_amount,
            "Calling fulfillRedeem"
        );

        self.fulfiller
            .fulfill_redeem(req.request_id as u64, collateral_amount)
            .await
    }
}

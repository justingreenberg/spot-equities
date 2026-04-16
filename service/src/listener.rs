use alloy::{
    primitives::Address,
    providers::{Provider, RootProvider},
    rpc::types::Filter,
    sol,
    sol_types::SolEvent,
};
use anyhow::Context;
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

use crate::db::queries;

// Generate Rust bindings for SpotVault events
sol! {
    event MintRequested(uint256 indexed requestId, address indexed requester, uint256 collateralAmount);
    event RedeemRequested(uint256 indexed requestId, address indexed requester, uint256 syntheticAmount);
}

pub struct EventListener {
    provider: RootProvider,
    vault_address: Address,
    pool: SqlitePool,
}

impl EventListener {
    pub fn new(
        provider: RootProvider,
        vault_address: Address,
        pool: SqlitePool,
    ) -> Self {
        Self {
            provider,
            vault_address,
            pool,
        }
    }

    /// Poll for new events since the last processed block.
    /// Returns the number of new events found.
    pub async fn poll(&self) -> anyhow::Result<usize> {
        let last_block = queries::get_block_cursor(&self.pool).await?;
        let current_block = self
            .provider
            .get_block_number()
            .await
            .context("Failed to get block number")? as i64;

        if current_block <= last_block {
            return Ok(0);
        }

        let from_block = (last_block + 1) as u64;
        let to_block = current_block as u64;

        debug!(from_block, to_block, "Polling for events");

        let filter = Filter::new()
            .address(self.vault_address)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self
            .provider
            .get_logs(&filter)
            .await
            .context("Failed to get logs")?;

        let mut count = 0;

        for log in &logs {
            if log.topics().is_empty() {
                continue;
            }

            let topic0 = log.topics()[0];

            if topic0 == MintRequested::SIGNATURE_HASH {
                match MintRequested::decode_log(log.as_ref()) {
                    Ok(event) => {
                        let request_id = event.data.requestId.as_limbs()[0] as i64;
                        let requester = format!("{:#x}", event.data.requester);
                        let collateral = event.data.collateralAmount.to_string();

                        info!(request_id, requester, collateral, "MintRequested event");
                        queries::insert_request(
                            &self.pool,
                            request_id,
                            "mint",
                            &requester,
                            &collateral,
                            None,
                        )
                        .await?;
                        count += 1;
                    }
                    Err(e) => warn!("Failed to decode MintRequested: {}", e),
                }
            } else if topic0 == RedeemRequested::SIGNATURE_HASH {
                match RedeemRequested::decode_log(log.as_ref()) {
                    Ok(event) => {
                        let request_id = event.data.requestId.as_limbs()[0] as i64;
                        let requester = format!("{:#x}", event.data.requester);
                        let synthetic = event.data.syntheticAmount.to_string();

                        info!(request_id, requester, synthetic, "RedeemRequested event");
                        queries::insert_request(
                            &self.pool,
                            request_id,
                            "redeem",
                            &requester,
                            "0",
                            Some(&synthetic),
                        )
                        .await?;
                        count += 1;
                    }
                    Err(e) => warn!("Failed to decode RedeemRequested: {}", e),
                }
            }
        }

        queries::update_block_cursor(&self.pool, current_block).await?;

        if count > 0 {
            info!(count, from_block, to_block, "Processed new events");
        }

        Ok(count)
    }
}

use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::db::queries;
use crate::dinari::client::DinariClient;

/// Periodically checks the treasury health (backing ratio, funding levels).
pub struct TreasuryReconciler {
    pool: SqlitePool,
    dinari: DinariClient,
    ticker: String,
}

impl TreasuryReconciler {
    pub fn new(pool: SqlitePool, dinari: DinariClient, ticker: String) -> Self {
        Self {
            pool,
            dinari,
            ticker,
        }
    }

    /// Run a treasury health check and save a snapshot.
    pub async fn check(&self) -> anyhow::Result<()> {
        let balance = self.dinari.get_account_balance().await?;

        let dshares_held = balance
            .dshares
            .iter()
            .find(|d| d.ticker == self.ticker)
            .map(|d| d.shares.clone())
            .unwrap_or_else(|| "0".to_string());

        // TODO: Query on-chain for total synthetic supply
        let synthetic_outstanding = "0".to_string();

        // Calculate backing ratio
        let dshares_f: f64 = dshares_held.parse().unwrap_or(0.0);
        let synthetic_f: f64 = synthetic_outstanding.parse().unwrap_or(0.0);
        let backing_ratio = if synthetic_f > 0.0 {
            format!("{:.4}", dshares_f / synthetic_f)
        } else {
            "1.0000".to_string()
        };

        queries::insert_treasury_snapshot(
            &self.pool,
            &balance.usdc_balance,
            &dshares_held,
            &synthetic_outstanding,
            &backing_ratio,
        )
        .await?;

        info!(
            usdc_balance = %balance.usdc_balance,
            dshares_held = %dshares_held,
            backing_ratio = %backing_ratio,
            "Treasury snapshot saved"
        );

        // Alert if backing ratio drops below 1.0
        if dshares_f < synthetic_f && synthetic_f > 0.0 {
            warn!(
                backing_ratio = %backing_ratio,
                "ALERT: Treasury undercollateralized!"
            );
        }

        Ok(())
    }
}

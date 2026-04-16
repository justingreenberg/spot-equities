use chrono::Utc;
use sqlx::SqlitePool;

use super::models::{BlockCursor, KycRecord, Request, TreasurySnapshot};

// ── Block Cursor ──

pub async fn get_block_cursor(pool: &SqlitePool) -> anyhow::Result<i64> {
    let row = sqlx::query_as::<_, BlockCursor>("SELECT * FROM block_cursor WHERE id = 1")
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.last_processed_block).unwrap_or(0))
}

pub async fn update_block_cursor(pool: &SqlitePool, block: i64) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO block_cursor (id, last_processed_block, updated_at) VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET last_processed_block = ?1, updated_at = ?2",
    )
    .bind(block)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(())
}

// ── Requests ──

pub async fn insert_request(
    pool: &SqlitePool,
    request_id: i64,
    request_type: &str,
    requester: &str,
    collateral_amount: &str,
    synthetic_amount: Option<&str>,
) -> anyhow::Result<()> {
    let now = Utc::now();
    sqlx::query(
        "INSERT OR IGNORE INTO requests
         (request_id, request_type, requester, collateral_amount, synthetic_amount, status, retry_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'detected', 0, ?6, ?6)",
    )
    .bind(request_id)
    .bind(request_type)
    .bind(requester)
    .bind(collateral_amount)
    .bind(synthetic_amount)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_requests_by_status(pool: &SqlitePool, status: &str) -> anyhow::Result<Vec<Request>> {
    let rows = sqlx::query_as::<_, Request>("SELECT * FROM requests WHERE status = ?1 ORDER BY created_at ASC")
        .bind(status)
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

pub async fn get_request_by_request_id(pool: &SqlitePool, request_id: i64) -> anyhow::Result<Option<Request>> {
    let row = sqlx::query_as::<_, Request>("SELECT * FROM requests WHERE request_id = ?1")
        .bind(request_id)
        .fetch_optional(pool)
        .await?;

    Ok(row)
}

pub async fn update_request_status(
    pool: &SqlitePool,
    request_id: i64,
    status: &str,
    dinari_order_id: Option<&str>,
    dinari_status: Option<&str>,
    dinari_fill_price: Option<&str>,
    dinari_fill_shares: Option<&str>,
    onchain_tx_hash: Option<&str>,
    last_error: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE requests SET
            status = ?1,
            dinari_order_id = COALESCE(?2, dinari_order_id),
            dinari_status = COALESCE(?3, dinari_status),
            dinari_fill_price = COALESCE(?4, dinari_fill_price),
            dinari_fill_shares = COALESCE(?5, dinari_fill_shares),
            onchain_tx_hash = COALESCE(?6, onchain_tx_hash),
            last_error = ?7,
            updated_at = ?8
         WHERE request_id = ?9",
    )
    .bind(status)
    .bind(dinari_order_id)
    .bind(dinari_status)
    .bind(dinari_fill_price)
    .bind(dinari_fill_shares)
    .bind(onchain_tx_hash)
    .bind(last_error)
    .bind(Utc::now())
    .bind(request_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn increment_retry(pool: &SqlitePool, request_id: i64, error: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE requests SET retry_count = retry_count + 1, last_error = ?1, updated_at = ?2 WHERE request_id = ?3",
    )
    .bind(error)
    .bind(Utc::now())
    .bind(request_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_requests(
    pool: &SqlitePool,
    request_type: Option<&str>,
    status: Option<&str>,
    requester: Option<&str>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<Request>> {
    // Build query dynamically based on filters
    let mut query = String::from("SELECT * FROM requests WHERE 1=1");
    if request_type.is_some() {
        query.push_str(" AND request_type = ?1");
    }
    if status.is_some() {
        query.push_str(" AND status = ?2");
    }
    if requester.is_some() {
        query.push_str(" AND requester = ?3");
    }
    query.push_str(" ORDER BY created_at DESC LIMIT ?4 OFFSET ?5");

    let rows = sqlx::query_as::<_, Request>(&query)
        .bind(request_type)
        .bind(status)
        .bind(requester)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

// ── Treasury ──

pub async fn insert_treasury_snapshot(
    pool: &SqlitePool,
    dinari_usdc_balance: &str,
    dinari_dshares_held: &str,
    synthetic_outstanding: &str,
    backing_ratio: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO treasury_snapshots
         (dinari_usdc_balance, dinari_dshares_held, synthetic_outstanding, backing_ratio, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(dinari_usdc_balance)
    .bind(dinari_dshares_held)
    .bind(synthetic_outstanding)
    .bind(backing_ratio)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_latest_treasury_snapshot(pool: &SqlitePool) -> anyhow::Result<Option<TreasurySnapshot>> {
    let row = sqlx::query_as::<_, TreasurySnapshot>(
        "SELECT * FROM treasury_snapshots ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

// ── KYC ──

pub async fn upsert_kyc_record(
    pool: &SqlitePool,
    wallet_address: &str,
    dinari_account_id: Option<&str>,
    kyc_url: Option<&str>,
    status: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO kyc_records (wallet_address, dinari_account_id, kyc_url, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(wallet_address) DO UPDATE SET
            dinari_account_id = COALESCE(?2, kyc_records.dinari_account_id),
            kyc_url = COALESCE(?3, kyc_records.kyc_url),
            status = ?4,
            updated_at = ?5",
    )
    .bind(wallet_address)
    .bind(dinari_account_id)
    .bind(kyc_url)
    .bind(status)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_kyc_record(pool: &SqlitePool, wallet_address: &str) -> anyhow::Result<Option<KycRecord>> {
    let row = sqlx::query_as::<_, KycRecord>(
        "SELECT * FROM kyc_records WHERE wallet_address = ?1",
    )
    .bind(wallet_address)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn update_kyc_status(
    pool: &SqlitePool,
    wallet_address: &str,
    status: &str,
    submitted_at: Option<&str>,
    approved_at: Option<&str>,
    rejected_reason: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE kyc_records SET
            status = ?1,
            submitted_at = COALESCE(?2, submitted_at),
            approved_at = COALESCE(?3, approved_at),
            rejected_reason = ?4,
            updated_at = ?5
         WHERE wallet_address = ?6",
    )
    .bind(status)
    .bind(submitted_at)
    .bind(approved_at)
    .bind(rejected_reason)
    .bind(Utc::now())
    .bind(wallet_address)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn mark_role_granted(
    pool: &SqlitePool,
    wallet_address: &str,
    tx_hash: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE kyc_records SET
            role_granted = 1,
            role_granted_at = ?1,
            role_tx_hash = ?2,
            updated_at = ?1
         WHERE wallet_address = ?3",
    )
    .bind(Utc::now())
    .bind(tx_hash)
    .bind(wallet_address)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_kyc_records(
    pool: &SqlitePool,
    status: Option<&str>,
) -> anyhow::Result<Vec<KycRecord>> {
    let rows = if let Some(s) = status {
        sqlx::query_as::<_, KycRecord>(
            "SELECT * FROM kyc_records WHERE status = ?1 ORDER BY updated_at DESC",
        )
        .bind(s)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, KycRecord>(
            "SELECT * FROM kyc_records ORDER BY updated_at DESC",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows)
}

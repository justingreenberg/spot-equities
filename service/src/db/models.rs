use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(rename_all = "snake_case")]
pub enum RequestType {
    Mint,
    Redeem,
}

impl std::fmt::Display for RequestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestType::Mint => write!(f, "mint"),
            RequestType::Redeem => write!(f, "redeem"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(rename_all = "snake_case")]
pub enum RequestStatus {
    Detected,
    Pending,
    Processing,
    DinariCompleted,
    ReadyToFulfill,
    Fulfilled,
    FulfillmentFailed,
    Failed,
}

impl std::fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestStatus::Detected => write!(f, "detected"),
            RequestStatus::Pending => write!(f, "pending"),
            RequestStatus::Processing => write!(f, "processing"),
            RequestStatus::DinariCompleted => write!(f, "dinari_completed"),
            RequestStatus::ReadyToFulfill => write!(f, "ready_to_fulfill"),
            RequestStatus::Fulfilled => write!(f, "fulfilled"),
            RequestStatus::FulfillmentFailed => write!(f, "fulfillment_failed"),
            RequestStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Request {
    pub id: i64,
    pub request_id: i64,
    pub request_type: String,
    pub requester: String,
    pub collateral_amount: String,
    pub synthetic_amount: Option<String>,
    pub status: String,
    pub dinari_order_id: Option<String>,
    pub dinari_status: Option<String>,
    pub dinari_fill_price: Option<String>,
    pub dinari_fill_shares: Option<String>,
    pub onchain_tx_hash: Option<String>,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BlockCursor {
    pub id: i32,
    pub last_processed_block: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct KycRecord {
    pub id: i64,
    pub wallet_address: String,
    pub dinari_account_id: Option<String>,
    pub kyc_url: Option<String>,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_reason: Option<String>,
    pub role_granted: bool,
    pub role_granted_at: Option<DateTime<Utc>>,
    pub role_tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TreasurySnapshot {
    pub id: i64,
    pub dinari_usdc_balance: String,
    pub dinari_dshares_held: String,
    pub synthetic_outstanding: String,
    pub backing_ratio: String,
    pub created_at: DateTime<Utc>,
}

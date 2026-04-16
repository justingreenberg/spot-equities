use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DinariOrder {
    pub id: String,
    pub status: DinariOrderStatus,
    pub stock_ticker: String,
    pub side: String,
    pub requested_amount: Option<String>,
    pub filled_amount: Option<String>,
    pub filled_shares: Option<String>,
    pub average_price: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DinariOrderStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub usdc_balance: String,
    pub dshares: Vec<DShareBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DShareBalance {
    pub ticker: String,
    pub shares: String,
}

#[derive(Debug, Serialize)]
pub struct CreateOrderRequest {
    pub stock_ticker: String,
    pub side: String,
    pub amount: Option<String>,
    pub shares: Option<String>,
    pub idempotency_key: String,
}

// ── KYC Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycSession {
    /// Dinari account/customer ID
    pub id: String,
    /// URL to redirect the user to for KYC completion
    pub kyc_url: String,
    /// Current KYC status
    pub status: KycStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KycStatus {
    NotStarted,
    Pending,
    InReview,
    Approved,
    Rejected,
}

impl std::fmt::Display for KycStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KycStatus::NotStarted => write!(f, "not_started"),
            KycStatus::Pending => write!(f, "pending"),
            KycStatus::InReview => write!(f, "in_review"),
            KycStatus::Approved => write!(f, "approved"),
            KycStatus::Rejected => write!(f, "rejected"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CreateKycRequest {
    pub wallet_address: String,
    /// Redirect URL after KYC completion
    pub redirect_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycStatusResponse {
    pub id: String,
    pub status: KycStatus,
    pub rejected_reason: Option<String>,
}

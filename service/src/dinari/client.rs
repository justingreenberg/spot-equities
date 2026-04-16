use anyhow::Context;
use reqwest::Client;
use tracing::{info, warn};

use super::types::{
    AccountBalance, CreateKycRequest, CreateOrderRequest, DinariOrder, KycSession, KycStatusResponse,
};

pub struct DinariClient {
    base_url: String,
    api_key_id: String,
    api_secret: String,
    http: Client,
}

impl DinariClient {
    pub fn new(base_url: &str, api_key_id: &str, api_secret: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key_id: api_key_id.to_string(),
            api_secret: api_secret.to_string(),
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    fn auth_headers(&self) -> Vec<(&str, String)> {
        vec![
            ("X-API-Key-ID", self.api_key_id.clone()),
            ("X-API-Secret", self.api_secret.clone()),
        ]
    }

    /// Place a buy order for dShares (used in mint flow)
    pub async fn create_buy_order(
        &self,
        ticker: &str,
        usdc_amount: &str,
        idempotency_key: &str,
    ) -> anyhow::Result<DinariOrder> {
        info!(ticker, usdc_amount, idempotency_key, "Creating Dinari buy order");

        let body = CreateOrderRequest {
            stock_ticker: ticker.to_string(),
            side: "buy".to_string(),
            amount: Some(usdc_amount.to_string()),
            shares: None,
            idempotency_key: idempotency_key.to_string(),
        };

        let mut req = self
            .http
            .post(format!("{}/api/v1/orders", self.base_url))
            .json(&body);

        for (key, value) in self.auth_headers() {
            req = req.header(key, value);
        }

        let resp = req.send().await.context("Dinari API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, body, "Dinari buy order failed");
            anyhow::bail!("Dinari API error {}: {}", status, body);
        }

        let order: DinariOrder = resp.json().await.context("Failed to parse Dinari response")?;
        info!(order_id = %order.id, "Dinari buy order created");
        Ok(order)
    }

    /// Place a sell order for dShares (used in redeem flow)
    pub async fn create_sell_order(
        &self,
        ticker: &str,
        shares: &str,
        idempotency_key: &str,
    ) -> anyhow::Result<DinariOrder> {
        info!(ticker, shares, idempotency_key, "Creating Dinari sell order");

        let body = CreateOrderRequest {
            stock_ticker: ticker.to_string(),
            side: "sell".to_string(),
            amount: None,
            shares: Some(shares.to_string()),
            idempotency_key: idempotency_key.to_string(),
        };

        let mut req = self
            .http
            .post(format!("{}/api/v1/orders", self.base_url))
            .json(&body);

        for (key, value) in self.auth_headers() {
            req = req.header(key, value);
        }

        let resp = req.send().await.context("Dinari API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, body, "Dinari sell order failed");
            anyhow::bail!("Dinari API error {}: {}", status, body);
        }

        let order: DinariOrder = resp.json().await.context("Failed to parse Dinari response")?;
        info!(order_id = %order.id, "Dinari sell order created");
        Ok(order)
    }

    /// Check the status of an existing order
    pub async fn get_order(&self, order_id: &str) -> anyhow::Result<DinariOrder> {
        let mut req = self
            .http
            .get(format!("{}/api/v1/orders/{}", self.base_url, order_id));

        for (key, value) in self.auth_headers() {
            req = req.header(key, value);
        }

        let resp = req.send().await.context("Dinari API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Dinari API error {}: {}", status, body);
        }

        let order: DinariOrder = resp.json().await?;
        Ok(order)
    }

    /// Get account balance (USDC + dShares held)
    pub async fn get_account_balance(&self) -> anyhow::Result<AccountBalance> {
        let mut req = self
            .http
            .get(format!("{}/api/v1/account/balance", self.base_url));

        for (key, value) in self.auth_headers() {
            req = req.header(key, value);
        }

        let resp = req.send().await.context("Dinari API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Dinari API error {}: {}", status, body);
        }

        let balance: AccountBalance = resp.json().await?;
        Ok(balance)
    }

    // ── KYC Methods ──

    /// Initiate managed KYC for a wallet address.
    /// Returns a KYC session with a redirect URL to Dinari's hosted KYC flow.
    pub async fn create_kyc_session(
        &self,
        wallet_address: &str,
        redirect_url: Option<&str>,
    ) -> anyhow::Result<KycSession> {
        info!(wallet_address, "Creating Dinari KYC session");

        let body = CreateKycRequest {
            wallet_address: wallet_address.to_string(),
            redirect_url: redirect_url.map(|s| s.to_string()),
        };

        let mut req = self
            .http
            .post(format!("{}/api/v1/kyc/sessions", self.base_url))
            .json(&body);

        for (key, value) in self.auth_headers() {
            req = req.header(key, value);
        }

        let resp = req.send().await.context("Dinari KYC API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, body, "Dinari KYC session creation failed");
            anyhow::bail!("Dinari KYC API error {}: {}", status, body);
        }

        let session: KycSession = resp.json().await.context("Failed to parse KYC session")?;
        info!(account_id = %session.id, "Dinari KYC session created");
        Ok(session)
    }

    /// Check the KYC status for a given Dinari account ID.
    pub async fn get_kyc_status(&self, account_id: &str) -> anyhow::Result<KycStatusResponse> {
        let mut req = self
            .http
            .get(format!("{}/api/v1/kyc/{}", self.base_url, account_id));

        for (key, value) in self.auth_headers() {
            req = req.header(key, value);
        }

        let resp = req.send().await.context("Dinari KYC API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Dinari KYC API error {}: {}", status, body);
        }

        let status: KycStatusResponse = resp.json().await?;
        Ok(status)
    }
}

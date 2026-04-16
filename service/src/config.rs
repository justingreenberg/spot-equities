use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Database URL (SQLite for dev, Postgres for prod)
    #[serde(default = "default_database_url")]
    pub database_url: String,

    /// HyperEVM RPC endpoint
    pub hyperevm_rpc_url: String,

    /// SpotVault contract address on HyperEVM
    pub vault_contract_address: String,

    /// Operator private key (hot wallet for fulfillment txs)
    pub operator_private_key: String,

    /// Dinari API base URL
    pub dinari_api_url: String,

    /// Dinari API key ID
    pub dinari_api_key_id: String,

    /// Dinari API secret
    pub dinari_api_secret: String,

    /// Stock ticker (e.g. "QQQ")
    #[serde(default = "default_ticker")]
    pub ticker: String,

    /// Clerk JWKS URL for JWT validation
    pub clerk_jwks_url: String,

    /// Event poll interval in milliseconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,

    /// Settlement engine tick interval in milliseconds
    #[serde(default = "default_settlement_interval")]
    pub settlement_interval_ms: u64,

    /// HTTP server port
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_database_url() -> String {
    "sqlite:spot-equities.db?mode=rwc".to_string()
}

fn default_ticker() -> String {
    "QQQ".to_string()
}

fn default_poll_interval() -> u64 {
    2000
}

fn default_settlement_interval() -> u64 {
    5000
}

fn default_port() -> u16 {
    3100
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(envy::from_env::<Config>()?)
    }
}

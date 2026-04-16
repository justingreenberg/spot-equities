use alloy::{
    network::EthereumWallet,
    primitives::{Address, FixedBytes, U256},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
        },
        Identity, Provider, RootProvider,
    },
    signers::local::PrivateKeySigner,
    sol,
    sol_types::SolCall,
    transports::http::Http,
};
use anyhow::Context;
use tracing::{error, info};

// Generate Rust bindings for SpotVault contract calls
sol! {
    #[sol(rpc)]
    interface ISpotVault {
        function markMintProcessing(uint256 requestId, bytes32 dinariOrderId) external;
        function markRedeemProcessing(uint256 requestId, bytes32 dinariOrderId) external;
        function fulfillMint(uint256 requestId, uint256 syntheticAmount) external;
        function fulfillRedeem(uint256 requestId, uint256 collateralAmount) external;
        function failMint(uint256 requestId) external;
        function failRedeem(uint256 requestId) external;
        function depositRedemptionFunds(uint256 amount) external;
    }
}

type SignerProvider = FillProvider<
    JoinFill<
        JoinFill<Identity, JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>>,
        alloy::providers::fillers::WalletFiller<EthereumWallet>,
    >,
    RootProvider,
>;

/// Handles submitting on-chain transactions to the SpotVault contract.
pub struct Fulfiller {
    vault_address: Address,
    provider: SignerProvider,
}

impl Fulfiller {
    pub fn new(rpc_url: &str, vault_address: Address, private_key: &str) -> anyhow::Result<Self> {
        let signer: PrivateKeySigner = private_key.parse().context("Invalid operator private key")?;
        let wallet = EthereumWallet::from(signer);

        let provider = alloy::providers::ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(rpc_url.parse().context("Invalid RPC URL")?);

        Ok(Self {
            vault_address,
            provider,
        })
    }

    /// Send a transaction and wait for the receipt. Returns the tx hash.
    async fn send_tx(&self, calldata: Vec<u8>) -> anyhow::Result<String> {
        let tx = alloy::rpc::types::TransactionRequest::default()
            .to(self.vault_address)
            .input(calldata.into());

        let pending = self
            .provider
            .send_transaction(tx)
            .await
            .context("Failed to send transaction")?;

        info!(tx_hash = %pending.tx_hash(), "Transaction sent, waiting for confirmation");

        let receipt = pending
            .get_receipt()
            .await
            .context("Failed to get transaction receipt")?;

        if !receipt.status() {
            error!(tx_hash = %receipt.transaction_hash, "Transaction reverted");
            anyhow::bail!("Transaction reverted: {}", receipt.transaction_hash);
        }

        let hash = format!("{:#x}", receipt.transaction_hash);
        info!(tx_hash = %hash, "Transaction confirmed");
        Ok(hash)
    }

    pub async fn mark_mint_processing(
        &self,
        request_id: u64,
        dinari_order_id: &str,
    ) -> anyhow::Result<String> {
        let mut order_bytes = [0u8; 32];
        let id_bytes = dinari_order_id.as_bytes();
        let len = id_bytes.len().min(32);
        order_bytes[..len].copy_from_slice(&id_bytes[..len]);

        let calldata = ISpotVault::markMintProcessingCall {
            requestId: U256::from(request_id),
            dinariOrderId: FixedBytes::from(order_bytes),
        }
        .abi_encode();

        self.send_tx(calldata).await
    }

    pub async fn mark_redeem_processing(
        &self,
        request_id: u64,
        dinari_order_id: &str,
    ) -> anyhow::Result<String> {
        let mut order_bytes = [0u8; 32];
        let id_bytes = dinari_order_id.as_bytes();
        let len = id_bytes.len().min(32);
        order_bytes[..len].copy_from_slice(&id_bytes[..len]);

        let calldata = ISpotVault::markRedeemProcessingCall {
            requestId: U256::from(request_id),
            dinariOrderId: FixedBytes::from(order_bytes),
        }
        .abi_encode();

        self.send_tx(calldata).await
    }

    pub async fn fulfill_mint(
        &self,
        request_id: u64,
        synthetic_amount: U256,
    ) -> anyhow::Result<String> {
        let calldata = ISpotVault::fulfillMintCall {
            requestId: U256::from(request_id),
            syntheticAmount: synthetic_amount,
        }
        .abi_encode();

        self.send_tx(calldata).await
    }

    pub async fn fulfill_redeem(
        &self,
        request_id: u64,
        collateral_amount: U256,
    ) -> anyhow::Result<String> {
        let calldata = ISpotVault::fulfillRedeemCall {
            requestId: U256::from(request_id),
            collateralAmount: collateral_amount,
        }
        .abi_encode();

        self.send_tx(calldata).await
    }

    pub async fn fail_mint(&self, request_id: u64) -> anyhow::Result<String> {
        let calldata = ISpotVault::failMintCall {
            requestId: U256::from(request_id),
        }
        .abi_encode();

        self.send_tx(calldata).await
    }

    pub async fn fail_redeem(&self, request_id: u64) -> anyhow::Result<String> {
        let calldata = ISpotVault::failRedeemCall {
            requestId: U256::from(request_id),
        }
        .abi_encode();

        self.send_tx(calldata).await
    }
}

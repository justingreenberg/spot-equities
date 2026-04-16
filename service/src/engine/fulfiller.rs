use alloy::{
    primitives::{Address, Bytes, U256},
    sol,
    sol_types::SolCall,
};

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

/// Handles submitting on-chain transactions to the SpotVault contract.
pub struct Fulfiller {
    vault_address: Address,
}

impl Fulfiller {
    pub fn new(vault_address: Address) -> Self {
        Self { vault_address }
    }

    pub fn encode_mark_mint_processing(&self, request_id: U256, dinari_order_id: [u8; 32]) -> Bytes {
        let call = ISpotVault::markMintProcessingCall {
            requestId: request_id,
            dinariOrderId: dinari_order_id.into(),
        };
        Bytes::from(call.abi_encode())
    }

    pub fn encode_mark_redeem_processing(&self, request_id: U256, dinari_order_id: [u8; 32]) -> Bytes {
        let call = ISpotVault::markRedeemProcessingCall {
            requestId: request_id,
            dinariOrderId: dinari_order_id.into(),
        };
        Bytes::from(call.abi_encode())
    }

    pub fn encode_fulfill_mint(&self, request_id: U256, synthetic_amount: U256) -> Bytes {
        let call = ISpotVault::fulfillMintCall {
            requestId: request_id,
            syntheticAmount: synthetic_amount,
        };
        Bytes::from(call.abi_encode())
    }

    pub fn encode_fulfill_redeem(&self, request_id: U256, collateral_amount: U256) -> Bytes {
        let call = ISpotVault::fulfillRedeemCall {
            requestId: request_id,
            collateralAmount: collateral_amount,
        };
        Bytes::from(call.abi_encode())
    }

    pub fn encode_fail_mint(&self, request_id: U256) -> Bytes {
        let call = ISpotVault::failMintCall {
            requestId: request_id,
        };
        Bytes::from(call.abi_encode())
    }

    pub fn encode_fail_redeem(&self, request_id: U256) -> Bytes {
        let call = ISpotVault::failRedeemCall {
            requestId: request_id,
        };
        Bytes::from(call.abi_encode())
    }

    pub fn vault_address(&self) -> Address {
        self.vault_address
    }
}

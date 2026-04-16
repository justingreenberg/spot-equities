// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {SpotVault} from "../src/SpotVault.sol";

/**
 * @title GrantMarketMaker
 * @notice Admin script to grant MARKET_MAKER_ROLE to a KYC-approved wallet.
 *
 * === KYC → Role Grant Flow ===
 *
 * 1. Market maker visits the spot-equities UI at /[network]/onboarding
 * 2. They connect their wallet and click "Start KYC Verification"
 * 3. The UI calls POST /api/kyc/init which creates a Dinari managed KYC session
 * 4. The user is redirected to Dinari's KYC portal to complete verification
 * 5. After submission, KYC status moves to "pending" → "in_review" → "approved"
 * 6. An admin checks the KYC status:
 *    - UI: /[network]/treasury → KYC Management tab
 *    - API: GET /api/admin/kyc?status=approved
 *    - CLI: curl $SERVICE_URL/api/admin/kyc?status=approved
 *
 * 7. Admin verifies the wallet is KYC-approved, then runs this script:
 *
 *    forge script script/GrantMarketMaker.s.sol \
 *      --sig "run(address,address)" \
 *      <VAULT_ADDRESS> <MARKET_MAKER_ADDRESS> \
 *      --rpc-url $HYPEREVM_RPC_URL \
 *      --private-key $ADMIN_PRIVATE_KEY \
 *      --broadcast
 *
 * 8. After the tx confirms, record the role grant in the service:
 *
 *    curl -X POST $SERVICE_URL/api/admin/kyc/grant-role \
 *      -H "Content-Type: application/json" \
 *      -d '{"wallet_address": "<MM_ADDRESS>", "tx_hash": "<TX_HASH>"}'
 *
 * === Prerequisites ===
 * - The caller must hold DEFAULT_ADMIN_ROLE on the SpotVault
 * - The market maker's KYC must be approved in the service database
 * - ADMIN_PRIVATE_KEY must be the admin multisig signer (or EOA on testnet)
 *
 * === Revoking Access ===
 * To revoke a market maker's access:
 *
 *    forge script script/GrantMarketMaker.s.sol \
 *      --sig "revoke(address,address)" \
 *      <VAULT_ADDRESS> <MARKET_MAKER_ADDRESS> \
 *      --rpc-url $HYPEREVM_RPC_URL \
 *      --private-key $ADMIN_PRIVATE_KEY \
 *      --broadcast
 */
contract GrantMarketMaker is Script {
    function run(address vault, address marketMaker) external {
        SpotVault spotVault = SpotVault(vault);
        bytes32 role = spotVault.MARKET_MAKER_ROLE();

        // Verify the caller has admin role
        require(
            spotVault.hasRole(spotVault.DEFAULT_ADMIN_ROLE(), msg.sender),
            "Caller is not admin"
        );

        // Check if already granted
        if (spotVault.hasRole(role, marketMaker)) {
            console.log("MARKET_MAKER_ROLE already granted to:", marketMaker);
            return;
        }

        vm.startBroadcast();
        spotVault.grantRole(role, marketMaker);
        vm.stopBroadcast();

        console.log("MARKET_MAKER_ROLE granted to:", marketMaker);
        console.log("Vault:", vault);
        console.log("");
        console.log("Next step: Record the grant in the service:");
        console.log("  curl -X POST $SERVICE_URL/api/admin/kyc/grant-role \\");
        console.log("    -H 'Content-Type: application/json' \\");
        console.log("    -d '{\"wallet_address\": \"<MM_ADDRESS>\", \"tx_hash\": \"<TX_HASH>\"}'");
    }

    function revoke(address vault, address marketMaker) external {
        SpotVault spotVault = SpotVault(vault);
        bytes32 role = spotVault.MARKET_MAKER_ROLE();

        require(
            spotVault.hasRole(spotVault.DEFAULT_ADMIN_ROLE(), msg.sender),
            "Caller is not admin"
        );

        if (!spotVault.hasRole(role, marketMaker)) {
            console.log("MARKET_MAKER_ROLE not held by:", marketMaker);
            return;
        }

        vm.startBroadcast();
        spotVault.revokeRole(role, marketMaker);
        vm.stopBroadcast();

        console.log("MARKET_MAKER_ROLE revoked from:", marketMaker);
    }

    /// @notice Batch grant MARKET_MAKER_ROLE to multiple addresses
    function batchGrant(address vault, address[] calldata marketMakers) external {
        SpotVault spotVault = SpotVault(vault);
        bytes32 role = spotVault.MARKET_MAKER_ROLE();

        vm.startBroadcast();
        for (uint256 i = 0; i < marketMakers.length; i++) {
            if (!spotVault.hasRole(role, marketMakers[i])) {
                spotVault.grantRole(role, marketMakers[i]);
                console.log("Granted to:", marketMakers[i]);
            }
        }
        vm.stopBroadcast();
    }
}

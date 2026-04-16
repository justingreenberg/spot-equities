// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {SyntheticToken} from "../src/SyntheticToken.sol";
import {SpotVault} from "../src/SpotVault.sol";
import {MockPauserRegistry} from "../test/mocks/MockPauserRegistry.sol";
import {MockERC20} from "../test/mocks/MockERC20.sol";

/**
 * @title Deploy
 * @notice Deploys the full spot-equities stack.
 *
 * Local anvil:
 *   forge script script/Deploy.s.sol --sig "deployLocal()" \
 *     --rpc-url http://localhost:8545 \
 *     --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
 *     --broadcast
 */
contract Deploy is Script {
    function deployLocal() external {
        address deployer = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;
        address operator = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8;
        address mm = 0x90F79bf6EB2c4f870365E785982E1f101E93b906;

        vm.startBroadcast();

        MockERC20 usdc = new MockERC20("USD Coin", "USDC", 6);
        MockPauserRegistry pauserRegistry = new MockPauserRegistry();
        SyntheticToken tokenImpl = new SyntheticToken();
        SpotVault vaultImpl = new SpotVault();

        // Deploy token — deployer is temporary minter/burner
        SyntheticToken token = SyntheticToken(address(new ERC1967Proxy(
            address(tokenImpl),
            abi.encodeCall(SyntheticToken.initialize, (
                "Synthetic QQQ", "sQQQ", deployer, deployer, deployer, address(pauserRegistry)
            ))
        )));

        // Deploy vault — deployer is admin + manager
        SpotVault vault = SpotVault(address(new ERC1967Proxy(
            address(vaultImpl),
            abi.encodeCall(SpotVault.initialize, (
                address(token), address(usdc), address(pauserRegistry), deployer, operator, deployer
            ))
        )));

        // Wire roles
        token.grantRole(token.MINTER_ROLE(), address(vault));
        token.grantRole(token.BURNER_ROLE(), address(vault));
        vault.grantRole(vault.MARKET_MAKER_ROLE(), mm);
        vault.setPriceBounds(100e6, 1000e6);

        // Fund accounts for testing
        usdc.mint(mm, 10_000_000e6);
        usdc.mint(operator, 10_000_000e6);

        vm.stopBroadcast();

        console.log("USDC_ADDRESS=%s", address(usdc));
        console.log("PAUSER_REGISTRY_ADDRESS=%s", address(pauserRegistry));
        console.log("SYNTHETIC_TOKEN_ADDRESS=%s", address(token));
        console.log("VAULT_CONTRACT_ADDRESS=%s", address(vault));
    }
}

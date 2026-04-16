// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {SyntheticToken} from "../src/SyntheticToken.sol";
import {MockPauserRegistry} from "./mocks/MockPauserRegistry.sol";

contract SyntheticTokenTest is Test {
    SyntheticToken public token;
    MockPauserRegistry public pauserRegistry;

    address public admin = makeAddr("admin");
    address public minter = makeAddr("minter");
    address public burner = makeAddr("burner");
    address public user = makeAddr("user");

    function setUp() public {
        pauserRegistry = new MockPauserRegistry();

        SyntheticToken impl = new SyntheticToken();
        bytes memory initData = abi.encodeCall(
            SyntheticToken.initialize,
            ("Synthetic QQQ", "sQQQ", admin, minter, burner, address(pauserRegistry))
        );
        ERC1967Proxy proxy = new ERC1967Proxy(address(impl), initData);
        token = SyntheticToken(address(proxy));
    }

    /* ========== INITIALIZATION ========== */

    function test_initialize() public view {
        assertEq(token.name(), "Synthetic QQQ");
        assertEq(token.symbol(), "sQQQ");
        assertEq(token.decimals(), 18);
        assertTrue(token.hasRole(token.DEFAULT_ADMIN_ROLE(), admin));
        assertTrue(token.hasRole(token.MINTER_ROLE(), minter));
        assertTrue(token.hasRole(token.BURNER_ROLE(), burner));
    }

    function test_initialize_revertsZeroAdmin() public {
        SyntheticToken impl = new SyntheticToken();
        bytes memory initData = abi.encodeCall(
            SyntheticToken.initialize,
            ("Synthetic QQQ", "sQQQ", address(0), minter, burner, address(pauserRegistry))
        );
        vm.expectRevert("SyntheticToken: invalid admin");
        new ERC1967Proxy(address(impl), initData);
    }

    function test_initialize_revertsZeroMinter() public {
        SyntheticToken impl = new SyntheticToken();
        bytes memory initData = abi.encodeCall(
            SyntheticToken.initialize,
            ("Synthetic QQQ", "sQQQ", admin, address(0), burner, address(pauserRegistry))
        );
        vm.expectRevert("SyntheticToken: invalid minter");
        new ERC1967Proxy(address(impl), initData);
    }

    /* ========== MINTING ========== */

    function test_mint() public {
        vm.prank(minter);
        token.mint(user, 100e18);
        assertEq(token.balanceOf(user), 100e18);
    }

    function test_mint_revertsUnauthorized() public {
        vm.prank(user);
        vm.expectRevert();
        token.mint(user, 100e18);
    }

    /* ========== BURNING ========== */

    function test_burn() public {
        vm.prank(minter);
        token.mint(user, 100e18);

        vm.prank(burner);
        token.burn(user, 40e18);
        assertEq(token.balanceOf(user), 60e18);
    }

    function test_burn_revertsUnauthorized() public {
        vm.prank(minter);
        token.mint(user, 100e18);

        vm.prank(user);
        vm.expectRevert();
        token.burn(user, 40e18);
    }

    /* ========== PAUSE ========== */

    function test_pause_blocksTransfer() public {
        vm.prank(minter);
        token.mint(user, 100e18);

        pauserRegistry.pauseContract(address(token));

        vm.prank(user);
        vm.expectRevert("SyntheticToken: paused");
        token.transfer(admin, 50e18);
    }

    function test_pause_blocksMint() public {
        pauserRegistry.pauseContract(address(token));

        vm.prank(minter);
        vm.expectRevert("SyntheticToken: paused");
        token.mint(user, 100e18);
    }

    function test_pause_blocksBurn() public {
        vm.prank(minter);
        token.mint(user, 100e18);

        pauserRegistry.pauseContract(address(token));

        vm.prank(burner);
        vm.expectRevert("SyntheticToken: paused");
        token.burn(user, 50e18);
    }

    function test_unpause_resumesTransfers() public {
        vm.prank(minter);
        token.mint(user, 100e18);

        pauserRegistry.pauseContract(address(token));
        pauserRegistry.unpauseContract(address(token));

        vm.prank(user);
        token.transfer(admin, 50e18);
        assertEq(token.balanceOf(admin), 50e18);
    }

    /* ========== FUZZ ========== */

    function testFuzz_mintBurn(uint256 mintAmount, uint256 burnAmount) public {
        mintAmount = bound(mintAmount, 1, type(uint128).max);
        burnAmount = bound(burnAmount, 0, mintAmount);

        vm.prank(minter);
        token.mint(user, mintAmount);

        vm.prank(burner);
        token.burn(user, burnAmount);

        assertEq(token.balanceOf(user), mintAmount - burnAmount);
    }
}

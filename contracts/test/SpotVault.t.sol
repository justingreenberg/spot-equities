// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {SyntheticToken} from "../src/SyntheticToken.sol";
import {SpotVault} from "../src/SpotVault.sol";
import {ISpotVault} from "../src/interfaces/ISpotVault.sol";
import {MockPauserRegistry} from "./mocks/MockPauserRegistry.sol";
import {MockERC20} from "./mocks/MockERC20.sol";

contract SpotVaultTest is Test {
    SyntheticToken public token;
    SpotVault public vault;
    MockPauserRegistry public pauserRegistry;
    MockERC20 public usdc;

    address public admin = makeAddr("admin");
    address public operator = makeAddr("operator");
    address public manager = makeAddr("manager");
    address public mm = makeAddr("marketMaker");
    address public mm2 = makeAddr("marketMaker2");

    // QQQ ~$480, so price in USDC (6 dec) per token (18 dec) = 480e6
    uint256 constant PRICE_MIN = 450e6;
    uint256 constant PRICE_MAX = 520e6;

    function setUp() public {
        pauserRegistry = new MockPauserRegistry();
        usdc = new MockERC20("USD Coin", "USDC", 6);

        // Deploy SyntheticToken via proxy
        SyntheticToken tokenImpl = new SyntheticToken();

        // Deploy SpotVault via proxy (we need the address for token init)
        SpotVault vaultImpl = new SpotVault();
        // We need to predict the vault proxy address for the token init
        // Deploy token first with a temporary minter/burner, then update
        // Actually, let's deploy vault first with a placeholder, then set token
        // Simplest: deploy both, then wire roles

        // Deploy vault proxy
        bytes memory vaultInitData = abi.encodeCall(
            SpotVault.initialize,
            (address(1), address(usdc), address(pauserRegistry), admin, operator, manager)
        );
        ERC1967Proxy vaultProxy = new ERC1967Proxy(address(vaultImpl), vaultInitData);
        vault = SpotVault(address(vaultProxy));

        // Deploy token proxy with vault as minter/burner
        bytes memory tokenInitData = abi.encodeCall(
            SyntheticToken.initialize,
            ("Synthetic QQQ", "sQQQ", admin, address(vault), address(vault), address(pauserRegistry))
        );
        ERC1967Proxy tokenProxy = new ERC1967Proxy(address(tokenImpl), tokenInitData);
        token = SyntheticToken(address(tokenProxy));

        // Re-deploy vault with correct token address
        // Since we can't re-initialize, let's use a fresh deploy
        vaultInitData = abi.encodeCall(
            SpotVault.initialize,
            (address(token), address(usdc), address(pauserRegistry), admin, operator, manager)
        );
        vaultProxy = new ERC1967Proxy(address(vaultImpl), vaultInitData);
        vault = SpotVault(address(vaultProxy));

        // Re-deploy token with the correct vault address
        tokenInitData = abi.encodeCall(
            SyntheticToken.initialize,
            ("Synthetic QQQ", "sQQQ", admin, address(vault), address(vault), address(pauserRegistry))
        );
        tokenProxy = new ERC1967Proxy(address(tokenImpl), tokenInitData);
        token = SyntheticToken(address(tokenProxy));

        // Re-deploy vault with the final token address
        vaultInitData = abi.encodeCall(
            SpotVault.initialize,
            (address(token), address(usdc), address(pauserRegistry), admin, operator, manager)
        );
        vaultProxy = new ERC1967Proxy(address(vaultImpl), vaultInitData);
        vault = SpotVault(address(vaultProxy));

        // Grant token roles to the final vault
        vm.startPrank(admin);
        token.grantRole(token.MINTER_ROLE(), address(vault));
        token.grantRole(token.BURNER_ROLE(), address(vault));
        vm.stopPrank();

        // Grant market maker role
        vm.startPrank(admin);
        vault.grantRole(vault.MARKET_MAKER_ROLE(), mm);
        vault.grantRole(vault.MARKET_MAKER_ROLE(), mm2);
        vm.stopPrank();

        // Set price bounds
        vm.prank(manager);
        vault.setPriceBounds(PRICE_MIN, PRICE_MAX);

        // Fund market maker with USDC
        usdc.mint(mm, 1_000_000e6);
        usdc.mint(mm2, 1_000_000e6);

        // Approve vault
        vm.prank(mm);
        usdc.approve(address(vault), type(uint256).max);
        vm.prank(mm2);
        usdc.approve(address(vault), type(uint256).max);
    }

    /* ========== HELPERS ========== */

    function _requestMint(address _mm, uint256 amount) internal returns (uint256) {
        vm.prank(_mm);
        return vault.requestMint(amount);
    }

    function _markMintProcessing(uint256 requestId) internal {
        vm.prank(operator);
        vault.markMintProcessing(requestId, bytes32("dinari-123"));
    }

    function _fulfillMint(uint256 requestId, uint256 syntheticAmount) internal {
        vm.prank(operator);
        vault.fulfillMint(requestId, syntheticAmount);
    }

    function _requestRedeem(address _mm, uint256 amount) internal returns (uint256) {
        vm.prank(_mm);
        return vault.requestRedeem(amount);
    }

    function _markRedeemProcessing(uint256 requestId) internal {
        vm.prank(operator);
        vault.markRedeemProcessing(requestId, bytes32("dinari-456"));
    }

    /* ========== INITIALIZATION ========== */

    function test_initialize() public view {
        assertTrue(vault.hasRole(vault.DEFAULT_ADMIN_ROLE(), admin));
        assertTrue(vault.hasRole(vault.OPERATOR_ROLE(), operator));
        assertTrue(vault.hasRole(vault.MANAGER_ROLE(), manager));
        assertEq(address(vault.syntheticToken()), address(token));
        assertEq(address(vault.collateralToken()), address(usdc));
        assertEq(vault.stalenessThreshold(), 24 hours);
    }

    /* ========== REQUEST MINT ========== */

    function test_requestMint() public {
        uint256 balBefore = usdc.balanceOf(mm);
        uint256 requestId = _requestMint(mm, 48_000e6); // ~100 QQQ worth

        assertEq(requestId, 0);
        assertEq(usdc.balanceOf(mm), balBefore - 48_000e6);
        assertEq(vault.totalCollateralLocked(), 48_000e6);

        (address requester, uint256 collateral,, uint256 ts, ISpotVault.RequestStatus status,) =
            vault.mintRequests(0);
        assertEq(requester, mm);
        assertEq(collateral, 48_000e6);
        assertEq(ts, block.timestamp);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Pending));
    }

    function test_requestMint_incrementsId() public {
        uint256 id0 = _requestMint(mm, 1000e6);
        uint256 id1 = _requestMint(mm, 2000e6);
        assertEq(id0, 0);
        assertEq(id1, 1);
    }

    function test_requestMint_revertsUnauthorized() public {
        address rando = makeAddr("rando");
        usdc.mint(rando, 1000e6);
        vm.startPrank(rando);
        usdc.approve(address(vault), type(uint256).max);
        vm.expectRevert();
        vault.requestMint(1000e6);
        vm.stopPrank();
    }

    function test_requestMint_revertsZeroAmount() public {
        vm.prank(mm);
        vm.expectRevert("SpotVault: zero amount");
        vault.requestMint(0);
    }

    function test_requestMint_revertsExceedsMax() public {
        vm.prank(manager);
        vault.setMaxMintAmount(10_000e6);

        vm.prank(mm);
        vm.expectRevert("SpotVault: exceeds max mint");
        vault.requestMint(10_001e6);
    }

    function test_requestMint_revertsWhenPaused() public {
        pauserRegistry.pauseContract(address(vault));

        vm.prank(mm);
        vm.expectRevert("SpotVault: paused");
        vault.requestMint(1000e6);
    }

    /* ========== MARK PROCESSING ========== */

    function test_markMintProcessing() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);

        (,,,, ISpotVault.RequestStatus status, bytes32 orderId) = vault.mintRequests(0);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Processing));
        assertEq(orderId, bytes32("dinari-123"));
    }

    function test_markMintProcessing_revertsNotPending() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);

        vm.prank(operator);
        vm.expectRevert("SpotVault: not pending");
        vault.markMintProcessing(0, bytes32("another"));
    }

    /* ========== FULFILL MINT ========== */

    function test_fulfillMint() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);

        // 48000 USDC / 480 price = 100 tokens
        _fulfillMint(0, 100e18);

        assertEq(token.balanceOf(mm), 100e18);
        assertEq(vault.totalCollateralLocked(), 0);

        (,, uint256 syntheticAmount,, ISpotVault.RequestStatus status,) = vault.mintRequests(0);
        assertEq(syntheticAmount, 100e18);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Fulfilled));
    }

    function test_fulfillMint_revertsPriceBelowMin() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);

        // Try to mint too many tokens (price would be below min)
        // 48000e6 * 1e18 / syntheticAmount >= 450e6
        // syntheticAmount <= 48000e6 * 1e18 / 450e6 = ~106.67e18
        vm.prank(operator);
        vm.expectRevert("SpotVault: price below min");
        vault.fulfillMint(0, 200e18); // price = 240e6, below 450e6
    }

    function test_fulfillMint_revertsPriceAboveMax() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);

        // Try to mint too few tokens (price would be above max)
        // 48000e6 * 1e18 / syntheticAmount <= 520e6
        // syntheticAmount >= 48000e6 * 1e18 / 520e6 = ~92.3e18
        vm.prank(operator);
        vm.expectRevert("SpotVault: price above max");
        vault.fulfillMint(0, 10e18); // price = 4800e6, above 520e6
    }

    function test_fulfillMint_revertsDoubleFullfill() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);
        _fulfillMint(0, 100e18);

        vm.prank(operator);
        vm.expectRevert("SpotVault: not processing");
        vault.fulfillMint(0, 100e18);
    }

    function test_fulfillMint_revertsNotProcessing() public {
        _requestMint(mm, 48_000e6);

        vm.prank(operator);
        vm.expectRevert("SpotVault: not processing");
        vault.fulfillMint(0, 100e18);
    }

    /* ========== FAIL MINT ========== */

    function test_failMint_refundsCollateral() public {
        uint256 balBefore = usdc.balanceOf(mm);
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);

        vm.prank(operator);
        vault.failMint(0);

        assertEq(usdc.balanceOf(mm), balBefore);
        assertEq(vault.totalCollateralLocked(), 0);

        (,,,, ISpotVault.RequestStatus status,) = vault.mintRequests(0);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Failed));
    }

    function test_failMint_fromPending() public {
        _requestMint(mm, 48_000e6);

        vm.prank(operator);
        vault.failMint(0);

        (,,,, ISpotVault.RequestStatus status,) = vault.mintRequests(0);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Failed));
    }

    function test_failMint_revertsAlreadyFulfilled() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);
        _fulfillMint(0, 100e18);

        vm.prank(operator);
        vm.expectRevert("SpotVault: cannot fail");
        vault.failMint(0);
    }

    /* ========== REQUEST REDEEM ========== */

    function test_requestRedeem() public {
        // Mint tokens first
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);
        _fulfillMint(0, 100e18);

        // Redeem
        uint256 requestId = _requestRedeem(mm, 50e18);
        assertEq(requestId, 0);
        assertEq(token.balanceOf(mm), 50e18); // 50 burned

        (address requester, uint256 syntheticAmount,, uint256 ts, ISpotVault.RequestStatus status,) =
            vault.redeemRequests(0);
        assertEq(requester, mm);
        assertEq(syntheticAmount, 50e18);
        assertEq(ts, block.timestamp);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Pending));
    }

    function test_requestRedeem_revertsInsufficientBalance() public {
        // MM has no synthetic tokens
        vm.prank(mm);
        vm.expectRevert();
        vault.requestRedeem(50e18);
    }

    /* ========== FULFILL REDEEM ========== */

    function test_fulfillRedeem() public {
        // Mint then redeem
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);
        _fulfillMint(0, 100e18);

        _requestRedeem(mm, 50e18);
        _markRedeemProcessing(0);

        // Operator deposits USDC for redemption payout
        usdc.mint(operator, 24_000e6);
        vm.startPrank(operator);
        usdc.approve(address(vault), type(uint256).max);
        vault.depositRedemptionFunds(24_000e6);

        // Fulfill redeem: 50 tokens * ~$480 = 24000 USDC
        uint256 mmBalBefore = usdc.balanceOf(mm);
        vault.fulfillRedeem(0, 24_000e6);
        vm.stopPrank();

        assertEq(usdc.balanceOf(mm), mmBalBefore + 24_000e6);

        (,, uint256 collateral,, ISpotVault.RequestStatus status,) = vault.redeemRequests(0);
        assertEq(collateral, 24_000e6);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Fulfilled));
    }

    /* ========== FAIL REDEEM ========== */

    function test_failRedeem_reMintTokens() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);
        _fulfillMint(0, 100e18);

        _requestRedeem(mm, 50e18);
        assertEq(token.balanceOf(mm), 50e18);

        vm.prank(operator);
        vault.failRedeem(0);

        // Tokens re-minted
        assertEq(token.balanceOf(mm), 100e18);

        (,,,, ISpotVault.RequestStatus status,) = vault.redeemRequests(0);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Failed));
    }

    /* ========== CANCEL STALE ========== */

    function test_cancelStaleMint() public {
        uint256 balBefore = usdc.balanceOf(mm);
        _requestMint(mm, 48_000e6);

        // Warp past staleness threshold
        vm.warp(block.timestamp + 25 hours);

        vm.prank(manager);
        vault.cancelStaleMint(0);

        assertEq(usdc.balanceOf(mm), balBefore);

        (,,,, ISpotVault.RequestStatus status,) = vault.mintRequests(0);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Cancelled));
    }

    function test_cancelStaleMint_revertsNotStale() public {
        _requestMint(mm, 48_000e6);

        vm.prank(manager);
        vm.expectRevert("SpotVault: not stale");
        vault.cancelStaleMint(0);
    }

    function test_cancelStaleRedeem() public {
        _requestMint(mm, 48_000e6);
        _markMintProcessing(0);
        _fulfillMint(0, 100e18);

        _requestRedeem(mm, 50e18);

        vm.warp(block.timestamp + 25 hours);

        vm.prank(manager);
        vault.cancelStaleRedeem(0);

        // Tokens re-minted
        assertEq(token.balanceOf(mm), 100e18);

        (,,,, ISpotVault.RequestStatus status,) = vault.redeemRequests(0);
        assertEq(uint8(status), uint8(ISpotVault.RequestStatus.Cancelled));
    }

    /* ========== MANAGER CONFIG ========== */

    function test_setPriceBounds() public {
        vm.prank(manager);
        vault.setPriceBounds(400e6, 600e6);
        assertEq(vault.mintPriceMin(), 400e6);
        assertEq(vault.mintPriceMax(), 600e6);
    }

    function test_setPriceBounds_revertsInvalid() public {
        vm.prank(manager);
        vm.expectRevert("SpotVault: invalid bounds");
        vault.setPriceBounds(600e6, 400e6);
    }

    function test_setStalenessThreshold() public {
        vm.prank(manager);
        vault.setStalenessThreshold(12 hours);
        assertEq(vault.stalenessThreshold(), 12 hours);
    }

    function test_setStalenessThreshold_revertsTooLow() public {
        vm.prank(manager);
        vm.expectRevert("SpotVault: threshold too low");
        vault.setStalenessThreshold(30 minutes);
    }

    /* ========== EMERGENCY ========== */

    function test_emergencyWithdraw() public {
        // Put some USDC in the vault
        usdc.mint(address(vault), 10_000e6);

        vm.prank(admin);
        vault.emergencyWithdraw(address(usdc), 10_000e6);

        assertEq(usdc.balanceOf(admin), 10_000e6);
    }

    function test_emergencyWithdraw_revertsUnauthorized() public {
        vm.prank(operator);
        vm.expectRevert();
        vault.emergencyWithdraw(address(usdc), 1);
    }

    /* ========== AVAILABLE COLLATERAL ========== */

    function test_availableCollateral() public {
        // Deposit redemption funds
        usdc.mint(operator, 50_000e6);
        vm.startPrank(operator);
        usdc.approve(address(vault), type(uint256).max);
        vault.depositRedemptionFunds(50_000e6);
        vm.stopPrank();

        assertEq(vault.availableCollateral(), 50_000e6);

        // Lock some collateral via mint request
        _requestMint(mm, 20_000e6);

        // Available = total balance (70k) - locked (20k) = 50k
        assertEq(vault.availableCollateral(), 50_000e6);
    }

    /* ========== FULL LIFECYCLE ========== */

    function test_fullLifecycle_mintThenRedeem() public {
        uint256 mmUsdcBefore = usdc.balanceOf(mm);

        // 1. Mint
        uint256 mintId = _requestMint(mm, 48_000e6);
        _markMintProcessing(mintId);
        _fulfillMint(mintId, 100e18);

        assertEq(token.balanceOf(mm), 100e18);
        assertEq(usdc.balanceOf(mm), mmUsdcBefore - 48_000e6);

        // 2. Redeem all
        uint256 redeemId = _requestRedeem(mm, 100e18);
        _markRedeemProcessing(redeemId);

        // Operator funds redemption
        usdc.mint(operator, 49_000e6);
        vm.startPrank(operator);
        usdc.approve(address(vault), type(uint256).max);
        vault.depositRedemptionFunds(49_000e6);
        vault.fulfillRedeem(redeemId, 49_000e6);
        vm.stopPrank();

        assertEq(token.balanceOf(mm), 0);
        assertEq(usdc.balanceOf(mm), mmUsdcBefore - 48_000e6 + 49_000e6);
    }

    /* ========== FUZZ ========== */

    function testFuzz_mintFulfill(uint256 collateral, uint256 syntheticAmount) public {
        collateral = bound(collateral, 1e6, 500_000e6);
        // Synthetic amount must satisfy price bounds
        // price = collateral * 1e18 / syntheticAmount must be in [PRICE_MIN, PRICE_MAX]
        // syntheticAmount in [collateral * 1e18 / PRICE_MAX, collateral * 1e18 / PRICE_MIN]
        uint256 minSynthetic = (collateral * 1e18) / PRICE_MAX;
        uint256 maxSynthetic = (collateral * 1e18) / PRICE_MIN;
        if (minSynthetic == 0) minSynthetic = 1;
        syntheticAmount = bound(syntheticAmount, minSynthetic, maxSynthetic);

        usdc.mint(mm, collateral);

        uint256 requestId = _requestMint(mm, collateral);
        _markMintProcessing(requestId);
        _fulfillMint(requestId, syntheticAmount);

        assertEq(token.balanceOf(mm), syntheticAmount);
    }
}

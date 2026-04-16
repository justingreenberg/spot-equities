// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

import {AccessControlEnumerableUpgradeable} from
    "@openzeppelin/contracts-upgradeable/access/extensions/AccessControlEnumerableUpgradeable.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

import {IPauserRegistry} from "./interfaces/IPauserRegistry.sol";
import {ISpotVault} from "./interfaces/ISpotVault.sol";
import {SyntheticToken} from "./SyntheticToken.sol";

/// @title SpotVault
/// @notice Manages the mint/redeem lifecycle for synthetic equity tokens backed by Dinari dShares.
///         Market makers deposit USDC to mint synthetic tokens; an off-chain operator fulfills
///         requests after purchasing dShares via the Dinari API.
contract SpotVault is ISpotVault, Initializable, AccessControlEnumerableUpgradeable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /* ========== ROLE DEFINITIONS ========== */

    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant MARKET_MAKER_ROLE = keccak256("MARKET_MAKER_ROLE");
    bytes32 public constant MANAGER_ROLE = keccak256("MANAGER_ROLE");

    /* ========== STATE VARIABLES ========== */

    SyntheticToken public syntheticToken;
    IERC20 public collateralToken;
    IPauserRegistry public pauserRegistry;

    uint256 public nextMintRequestId;
    uint256 public nextRedeemRequestId;

    mapping(uint256 => MintRequest) public mintRequests;
    mapping(uint256 => RedeemRequest) public redeemRequests;

    // Price bounds: collateral per synthetic token (6 decimal USDC per 18 decimal synthetic)
    uint256 public mintPriceMin;
    uint256 public mintPriceMax;

    // Per-request caps
    uint256 public maxMintAmount;
    uint256 public maxRedeemAmount;

    // Staleness threshold for cancellation (seconds)
    uint256 public stalenessThreshold;

    // Accounting
    uint256 public totalCollateralLocked;

    /* ========== MODIFIERS ========== */

    modifier whenNotPaused() {
        require(!pauserRegistry.isPaused(address(this)), "SpotVault: paused");
        _;
    }

    /* ========== INITIALIZATION ========== */

    /// @notice Initializes the vault
    /// @param _syntheticToken The synthetic equity token this vault manages
    /// @param _collateralToken The collateral token (USDC)
    /// @param _pauserRegistry PauserRegistry contract
    /// @param _admin Address receiving DEFAULT_ADMIN_ROLE
    /// @param _operator Address receiving OPERATOR_ROLE (Rust service hot wallet)
    /// @param _manager Address receiving MANAGER_ROLE
    function initialize(
        address _syntheticToken,
        address _collateralToken,
        address _pauserRegistry,
        address _admin,
        address _operator,
        address _manager
    ) public initializer {
        require(_syntheticToken != address(0), "SpotVault: invalid synthetic token");
        require(_collateralToken != address(0), "SpotVault: invalid collateral token");
        require(_pauserRegistry != address(0), "SpotVault: invalid pauser registry");
        require(_admin != address(0), "SpotVault: invalid admin");
        require(_operator != address(0), "SpotVault: invalid operator");
        require(_manager != address(0), "SpotVault: invalid manager");

        __AccessControlEnumerable_init();

        syntheticToken = SyntheticToken(_syntheticToken);
        collateralToken = IERC20(_collateralToken);
        pauserRegistry = IPauserRegistry(_pauserRegistry);

        _setRoleAdmin(OPERATOR_ROLE, DEFAULT_ADMIN_ROLE);
        _setRoleAdmin(MARKET_MAKER_ROLE, DEFAULT_ADMIN_ROLE);
        _setRoleAdmin(MANAGER_ROLE, DEFAULT_ADMIN_ROLE);

        _grantRole(DEFAULT_ADMIN_ROLE, _admin);
        _grantRole(OPERATOR_ROLE, _operator);
        _grantRole(MANAGER_ROLE, _manager);

        stalenessThreshold = 24 hours;
    }

    /* ========== MARKET MAKER FUNCTIONS ========== */

    /// @inheritdoc ISpotVault
    function requestMint(uint256 collateralAmount)
        external
        onlyRole(MARKET_MAKER_ROLE)
        nonReentrant
        whenNotPaused
        returns (uint256 requestId)
    {
        require(collateralAmount > 0, "SpotVault: zero amount");
        require(maxMintAmount == 0 || collateralAmount <= maxMintAmount, "SpotVault: exceeds max mint");

        collateralToken.safeTransferFrom(msg.sender, address(this), collateralAmount);

        requestId = nextMintRequestId++;
        mintRequests[requestId] = MintRequest({
            requester: msg.sender,
            collateralAmount: collateralAmount,
            syntheticAmount: 0,
            timestamp: block.timestamp,
            status: RequestStatus.Pending,
            dinariOrderId: bytes32(0)
        });

        totalCollateralLocked += collateralAmount;

        emit MintRequested(requestId, msg.sender, collateralAmount);
    }

    /// @inheritdoc ISpotVault
    function requestRedeem(uint256 syntheticAmount)
        external
        onlyRole(MARKET_MAKER_ROLE)
        nonReentrant
        whenNotPaused
        returns (uint256 requestId)
    {
        require(syntheticAmount > 0, "SpotVault: zero amount");
        require(maxRedeemAmount == 0 || syntheticAmount <= maxRedeemAmount, "SpotVault: exceeds max redeem");

        // Burn synthetic tokens from the market maker
        syntheticToken.burn(msg.sender, syntheticAmount);

        requestId = nextRedeemRequestId++;
        redeemRequests[requestId] = RedeemRequest({
            requester: msg.sender,
            syntheticAmount: syntheticAmount,
            collateralAmount: 0,
            timestamp: block.timestamp,
            status: RequestStatus.Pending,
            dinariOrderId: bytes32(0)
        });

        emit RedeemRequested(requestId, msg.sender, syntheticAmount);
    }

    /* ========== OPERATOR FUNCTIONS ========== */

    /// @inheritdoc ISpotVault
    function markMintProcessing(uint256 requestId, bytes32 dinariOrderId)
        external
        onlyRole(OPERATOR_ROLE)
    {
        MintRequest storage req = mintRequests[requestId];
        require(req.status == RequestStatus.Pending, "SpotVault: not pending");

        req.status = RequestStatus.Processing;
        req.dinariOrderId = dinariOrderId;

        emit MintProcessing(requestId, dinariOrderId);
    }

    /// @inheritdoc ISpotVault
    function markRedeemProcessing(uint256 requestId, bytes32 dinariOrderId)
        external
        onlyRole(OPERATOR_ROLE)
    {
        RedeemRequest storage req = redeemRequests[requestId];
        require(req.status == RequestStatus.Pending, "SpotVault: not pending");

        req.status = RequestStatus.Processing;
        req.dinariOrderId = dinariOrderId;

        emit RedeemProcessing(requestId, dinariOrderId);
    }

    /// @inheritdoc ISpotVault
    function fulfillMint(uint256 requestId, uint256 syntheticAmount)
        external
        onlyRole(OPERATOR_ROLE)
        nonReentrant
    {
        MintRequest storage req = mintRequests[requestId];
        require(req.status == RequestStatus.Processing, "SpotVault: not processing");
        require(syntheticAmount > 0, "SpotVault: zero synthetic amount");

        // Price sanity check: collateralAmount / syntheticAmount should be within bounds
        // Both amounts use different decimals (USDC=6, synthetic=18), so we compute
        // price = collateralAmount * 1e18 / syntheticAmount (result in 6 decimals)
        if (mintPriceMin > 0 || mintPriceMax > 0) {
            uint256 price = (req.collateralAmount * 1e18) / syntheticAmount;
            require(mintPriceMin == 0 || price >= mintPriceMin, "SpotVault: price below min");
            require(mintPriceMax == 0 || price <= mintPriceMax, "SpotVault: price above max");
        }

        req.status = RequestStatus.Fulfilled;
        req.syntheticAmount = syntheticAmount;
        totalCollateralLocked -= req.collateralAmount;

        syntheticToken.mint(req.requester, syntheticAmount);

        emit MintFulfilled(requestId, syntheticAmount);
    }

    /// @inheritdoc ISpotVault
    function fulfillRedeem(uint256 requestId, uint256 collateralAmount)
        external
        onlyRole(OPERATOR_ROLE)
        nonReentrant
    {
        RedeemRequest storage req = redeemRequests[requestId];
        require(req.status == RequestStatus.Processing, "SpotVault: not processing");
        require(collateralAmount > 0, "SpotVault: zero collateral amount");

        // Price sanity check for redeem
        if (mintPriceMin > 0 || mintPriceMax > 0) {
            uint256 price = (collateralAmount * 1e18) / req.syntheticAmount;
            require(mintPriceMin == 0 || price >= mintPriceMin, "SpotVault: price below min");
            require(mintPriceMax == 0 || price <= mintPriceMax, "SpotVault: price above max");
        }

        req.status = RequestStatus.Fulfilled;
        req.collateralAmount = collateralAmount;

        collateralToken.safeTransfer(req.requester, collateralAmount);

        emit RedeemFulfilled(requestId, collateralAmount);
    }

    /// @inheritdoc ISpotVault
    function failMint(uint256 requestId) external onlyRole(OPERATOR_ROLE) nonReentrant {
        MintRequest storage req = mintRequests[requestId];
        require(
            req.status == RequestStatus.Pending || req.status == RequestStatus.Processing,
            "SpotVault: cannot fail"
        );

        req.status = RequestStatus.Failed;
        totalCollateralLocked -= req.collateralAmount;

        // Refund locked USDC to market maker
        collateralToken.safeTransfer(req.requester, req.collateralAmount);

        emit MintFailed(requestId);
    }

    /// @inheritdoc ISpotVault
    function failRedeem(uint256 requestId) external onlyRole(OPERATOR_ROLE) nonReentrant {
        RedeemRequest storage req = redeemRequests[requestId];
        require(
            req.status == RequestStatus.Pending || req.status == RequestStatus.Processing,
            "SpotVault: cannot fail"
        );

        req.status = RequestStatus.Failed;

        // Re-mint the burned synthetic tokens back to market maker
        syntheticToken.mint(req.requester, req.syntheticAmount);

        emit RedeemFailed(requestId);
    }

    /// @inheritdoc ISpotVault
    function depositRedemptionFunds(uint256 amount) external onlyRole(OPERATOR_ROLE) nonReentrant {
        require(amount > 0, "SpotVault: zero amount");
        collateralToken.safeTransferFrom(msg.sender, address(this), amount);
        emit RedemptionFundsDeposited(amount);
    }

    /* ========== MANAGER FUNCTIONS ========== */

    /// @inheritdoc ISpotVault
    function setPriceBounds(uint256 minPrice, uint256 maxPrice) external onlyRole(MANAGER_ROLE) {
        require(maxPrice == 0 || minPrice <= maxPrice, "SpotVault: invalid bounds");
        mintPriceMin = minPrice;
        mintPriceMax = maxPrice;
        emit PriceBoundsUpdated(minPrice, maxPrice);
    }

    /// @inheritdoc ISpotVault
    function setMaxMintAmount(uint256 amount) external onlyRole(MANAGER_ROLE) {
        maxMintAmount = amount;
        emit MaxMintAmountUpdated(amount);
    }

    /// @inheritdoc ISpotVault
    function setMaxRedeemAmount(uint256 amount) external onlyRole(MANAGER_ROLE) {
        maxRedeemAmount = amount;
        emit MaxRedeemAmountUpdated(amount);
    }

    /// @inheritdoc ISpotVault
    function setStalenessThreshold(uint256 threshold) external onlyRole(MANAGER_ROLE) {
        require(threshold >= 1 hours, "SpotVault: threshold too low");
        stalenessThreshold = threshold;
        emit StalenessThresholdUpdated(threshold);
    }

    /// @inheritdoc ISpotVault
    function cancelStaleMint(uint256 requestId) external onlyRole(MANAGER_ROLE) nonReentrant {
        MintRequest storage req = mintRequests[requestId];
        require(
            req.status == RequestStatus.Pending || req.status == RequestStatus.Processing,
            "SpotVault: cannot cancel"
        );
        require(block.timestamp >= req.timestamp + stalenessThreshold, "SpotVault: not stale");

        req.status = RequestStatus.Cancelled;
        totalCollateralLocked -= req.collateralAmount;

        collateralToken.safeTransfer(req.requester, req.collateralAmount);

        emit MintCancelled(requestId);
    }

    /// @inheritdoc ISpotVault
    function cancelStaleRedeem(uint256 requestId) external onlyRole(MANAGER_ROLE) nonReentrant {
        RedeemRequest storage req = redeemRequests[requestId];
        require(
            req.status == RequestStatus.Pending || req.status == RequestStatus.Processing,
            "SpotVault: cannot cancel"
        );
        require(block.timestamp >= req.timestamp + stalenessThreshold, "SpotVault: not stale");

        req.status = RequestStatus.Cancelled;

        // Re-mint burned tokens
        syntheticToken.mint(req.requester, req.syntheticAmount);

        emit RedeemCancelled(requestId);
    }

    /* ========== ADMIN FUNCTIONS ========== */

    /// @inheritdoc ISpotVault
    function emergencyWithdraw(address token, uint256 amount)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
        nonReentrant
    {
        IERC20(token).safeTransfer(msg.sender, amount);
    }

    /* ========== VIEW FUNCTIONS ========== */

    /// @notice Returns the vault's collateral balance available for redemptions
    ///         (total balance minus locked collateral for pending mints)
    function availableCollateral() external view returns (uint256) {
        uint256 balance = collateralToken.balanceOf(address(this));
        if (balance <= totalCollateralLocked) return 0;
        return balance - totalCollateralLocked;
    }
}

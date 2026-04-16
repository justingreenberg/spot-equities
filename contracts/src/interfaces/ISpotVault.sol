// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

interface ISpotVault {
    /* ========== ENUMS ========== */

    enum RequestStatus {
        Pending,
        Processing,
        Fulfilled,
        Failed,
        Cancelled
    }

    /* ========== STRUCTS ========== */

    struct MintRequest {
        address requester;
        uint256 collateralAmount;
        uint256 syntheticAmount;
        uint256 timestamp;
        RequestStatus status;
        bytes32 dinariOrderId;
    }

    struct RedeemRequest {
        address requester;
        uint256 syntheticAmount;
        uint256 collateralAmount;
        uint256 timestamp;
        RequestStatus status;
        bytes32 dinariOrderId;
    }

    /* ========== EVENTS ========== */

    event MintRequested(uint256 indexed requestId, address indexed requester, uint256 collateralAmount);
    event MintProcessing(uint256 indexed requestId, bytes32 dinariOrderId);
    event MintFulfilled(uint256 indexed requestId, uint256 syntheticAmount);
    event MintFailed(uint256 indexed requestId);
    event MintCancelled(uint256 indexed requestId);

    event RedeemRequested(uint256 indexed requestId, address indexed requester, uint256 syntheticAmount);
    event RedeemProcessing(uint256 indexed requestId, bytes32 dinariOrderId);
    event RedeemFulfilled(uint256 indexed requestId, uint256 collateralAmount);
    event RedeemFailed(uint256 indexed requestId);
    event RedeemCancelled(uint256 indexed requestId);

    event PriceBoundsUpdated(uint256 minPrice, uint256 maxPrice);
    event MaxMintAmountUpdated(uint256 amount);
    event MaxRedeemAmountUpdated(uint256 amount);
    event StalenessThresholdUpdated(uint256 threshold);
    event RedemptionFundsDeposited(uint256 amount);

    /* ========== MARKET MAKER FUNCTIONS ========== */

    function requestMint(uint256 collateralAmount) external returns (uint256 requestId);
    function requestRedeem(uint256 syntheticAmount) external returns (uint256 requestId);

    /* ========== OPERATOR FUNCTIONS ========== */

    function markMintProcessing(uint256 requestId, bytes32 dinariOrderId) external;
    function markRedeemProcessing(uint256 requestId, bytes32 dinariOrderId) external;
    function fulfillMint(uint256 requestId, uint256 syntheticAmount) external;
    function fulfillRedeem(uint256 requestId, uint256 collateralAmount) external;
    function failMint(uint256 requestId) external;
    function failRedeem(uint256 requestId) external;
    function depositRedemptionFunds(uint256 amount) external;

    /* ========== MANAGER FUNCTIONS ========== */

    function setPriceBounds(uint256 minPrice, uint256 maxPrice) external;
    function setMaxMintAmount(uint256 amount) external;
    function setMaxRedeemAmount(uint256 amount) external;
    function setStalenessThreshold(uint256 threshold) external;
    function cancelStaleMint(uint256 requestId) external;
    function cancelStaleRedeem(uint256 requestId) external;

    /* ========== ADMIN FUNCTIONS ========== */

    function emergencyWithdraw(address token, uint256 amount) external;
}

// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

interface IPauserRegistry {
    function isPaused(address contractAddress) external view returns (bool);
    function isAuthorizedContract(address contractAddress) external view returns (bool);
    function pauseContract(address contractAddress) external;
    function unpauseContract(address contractAddress) external;
    function emergencyPauseAll() external;
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IPauserRegistry} from "../../src/interfaces/IPauserRegistry.sol";

contract MockPauserRegistry is IPauserRegistry {
    mapping(address => bool) private _paused;

    function isPaused(address contractAddress) external view returns (bool) {
        return _paused[contractAddress];
    }

    function isAuthorizedContract(address) external pure returns (bool) {
        return true;
    }

    function pauseContract(address contractAddress) external {
        _paused[contractAddress] = true;
    }

    function unpauseContract(address contractAddress) external {
        _paused[contractAddress] = false;
    }

    function emergencyPauseAll() external {}
}

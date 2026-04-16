// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

interface ISyntheticToken {
    function initialize(
        string calldata name,
        string calldata symbol,
        address admin,
        address minter,
        address burner,
        address pauserRegistry
    ) external;

    function mint(address to, uint256 amount) external;
    function burn(address from, uint256 amount) external;
}

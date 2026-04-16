// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface ICoreWriter {
    function sendRawAction(bytes calldata data) external;
}

/// @title HyperCoreWriter
/// @notice Library for interacting with HyperCore system contracts from HyperEVM.
///         Adapted from HIP3L1Write. Available for future automation of spot token
///         transfers between HyperEVM and HyperCore.
library HyperCoreWriter {
    address constant CORE_WRITER = 0x3333333333333333333333333333333333333333;

    function _encodeAction(uint24 actionId, bytes memory data) internal pure returns (bytes memory) {
        return abi.encodePacked(uint8(1), actionId, data);
    }

    /// @notice Send spot tokens from HyperEVM to a HyperCore address
    /// @param destination The recipient address on HyperCore
    /// @param token The spot token ID on HyperCore
    /// @param _wei The amount in wei to send
    function sendSpot(address destination, uint64 token, uint64 _wei) internal {
        bytes memory actionData = abi.encode(destination, token, _wei);
        ICoreWriter(CORE_WRITER).sendRawAction(_encodeAction(6, actionData));
    }
}

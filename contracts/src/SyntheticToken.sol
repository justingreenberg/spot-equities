// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.20;

import {AccessControlEnumerableUpgradeable} from
    "@openzeppelin/contracts-upgradeable/access/extensions/AccessControlEnumerableUpgradeable.sol";
import {ERC20PermitUpgradeable} from
    "@openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol";
import {IPauserRegistry} from "./interfaces/IPauserRegistry.sol";

/// @title SyntheticToken
/// @notice ERC20 representing a synthetic equity (e.g. sQQQ) backed 1:1 by Dinari dShares.
///         Follows the KHYPE pattern: upgradeable, role-based mint/burn, pausable via PauserRegistry.
contract SyntheticToken is ERC20PermitUpgradeable, AccessControlEnumerableUpgradeable {
    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    /* ========== STATE VARIABLES ========== */

    IPauserRegistry public pauserRegistry;

    /* ========== ROLE DEFINITIONS ========== */

    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");

    /* ========== MODIFIERS ========== */

    modifier whenNotPaused() {
        require(!pauserRegistry.isPaused(address(this)), "SyntheticToken: paused");
        _;
    }

    /* ========== INITIALIZATION ========== */

    /// @notice Initializes the synthetic token
    /// @param name Token name (e.g. "Synthetic QQQ")
    /// @param symbol Token symbol (e.g. "sQQQ")
    /// @param admin Address receiving DEFAULT_ADMIN_ROLE
    /// @param minter Address receiving MINTER_ROLE (SpotVault)
    /// @param burner Address receiving BURNER_ROLE (SpotVault)
    /// @param _pauserRegistry PauserRegistry contract address
    function initialize(
        string calldata name,
        string calldata symbol,
        address admin,
        address minter,
        address burner,
        address _pauserRegistry
    ) public initializer {
        require(admin != address(0), "SyntheticToken: invalid admin");
        require(minter != address(0), "SyntheticToken: invalid minter");
        require(burner != address(0), "SyntheticToken: invalid burner");
        require(_pauserRegistry != address(0), "SyntheticToken: invalid pauser registry");

        __ERC20_init(name, symbol);
        __ERC20Permit_init(name);
        __AccessControlEnumerable_init();

        _setRoleAdmin(MINTER_ROLE, DEFAULT_ADMIN_ROLE);
        _setRoleAdmin(BURNER_ROLE, DEFAULT_ADMIN_ROLE);

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(MINTER_ROLE, minter);
        _grantRole(BURNER_ROLE, burner);

        pauserRegistry = IPauserRegistry(_pauserRegistry);
    }

    /* ========== TOKEN OPERATIONS ========== */

    /// @notice Mints tokens to the specified address
    /// @param to Recipient address
    /// @param amount Amount to mint
    function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) {
        _mint(to, amount);
    }

    /// @notice Burns tokens from the specified address
    /// @param from Address to burn from
    /// @param amount Amount to burn
    function burn(address from, uint256 amount) external onlyRole(BURNER_ROLE) {
        _burn(from, amount);
    }

    /* ========== OVERRIDES ========== */

    /// @dev Pauses all transfers (including mint/burn) when the contract is paused
    function _update(address from, address to, uint256 value) internal virtual override whenNotPaused {
        super._update(from, to, value);
    }

    function supportsInterface(bytes4 interfaceId)
        public
        view
        virtual
        override(AccessControlEnumerableUpgradeable)
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
}

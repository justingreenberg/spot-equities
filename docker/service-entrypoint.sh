#!/usr/bin/env bash
set -euo pipefail

echo "Waiting for contract addresses..."
until [ -f /shared/addresses.env ]; do
  sleep 1
done

echo "Loading contract addresses..."
set -a
source /shared/addresses.env
set +a

echo "Vault:    $VAULT_CONTRACT_ADDRESS"
echo "RPC:      $HYPEREVM_RPC_URL"
echo "Database: $DATABASE_URL"
echo ""

echo "Starting spot-equities service..."
exec /usr/local/bin/spot-equities-service

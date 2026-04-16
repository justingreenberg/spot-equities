#!/usr/bin/env bash
set -uo pipefail

# Wait for Anvil to be ready
echo "Waiting for Anvil..."
until cast chain-id --rpc-url http://anvil:8545 &>/dev/null; do
  sleep 0.5
done
echo "Anvil is ready (chain-id: $(cast chain-id --rpc-url http://anvil:8545))"

# Anvil default deployer key (account[0])
DEPLOYER_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Deploy contracts
echo "Deploying contracts..."
forge script script/Deploy.s.sol \
  --sig "deployLocal()" \
  --rpc-url http://anvil:8545 \
  --private-key $DEPLOYER_KEY \
  --broadcast \
  --slow \
  2>&1 | tee /tmp/deploy-output.txt

DEPLOY_EXIT=${PIPESTATUS[0]}
if [ "$DEPLOY_EXIT" -ne 0 ]; then
  echo "ERROR: forge script failed with exit code $DEPLOY_EXIT"
  cat /tmp/deploy-output.txt
  exit 1
fi

OUTPUT=$(cat /tmp/deploy-output.txt)

# Parse deployed addresses from forge output
VAULT_ADDRESS=$(echo "$OUTPUT" | grep "VAULT_CONTRACT_ADDRESS=" | sed 's/.*VAULT_CONTRACT_ADDRESS=//')
TOKEN_ADDRESS=$(echo "$OUTPUT" | grep "SYNTHETIC_TOKEN_ADDRESS=" | sed 's/.*SYNTHETIC_TOKEN_ADDRESS=//')
USDC_ADDRESS=$(echo "$OUTPUT" | grep "USDC_ADDRESS=" | sed 's/.*USDC_ADDRESS=//')

if [ -z "$VAULT_ADDRESS" ] || [ -z "$TOKEN_ADDRESS" ]; then
  echo "ERROR: Failed to parse contract addresses from deploy output"
  echo "Full output:"
  echo "$OUTPUT"
  exit 1
fi

echo ""
echo "=== Writing /shared/addresses.env ==="
cat > /shared/addresses.env <<EOF
VAULT_CONTRACT_ADDRESS=$VAULT_ADDRESS
SYNTHETIC_TOKEN_ADDRESS=$TOKEN_ADDRESS
USDC_ADDRESS=$USDC_ADDRESS
HYPEREVM_RPC_URL=http://anvil:8545
OPERATOR_PRIVATE_KEY=0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
DATABASE_URL=sqlite:/data/spot-equities.db?mode=rwc
DINARI_API_URL=http://localhost:9999
DINARI_API_KEY_ID=test-key
DINARI_API_SECRET=test-secret
TICKER=QQQ
CLERK_JWKS_URL=http://localhost:9999/.well-known/jwks.json
POLL_INTERVAL_MS=2000
SETTLEMENT_INTERVAL_MS=5000
PORT=3100
EOF

echo "Vault:  $VAULT_ADDRESS"
echo "Token:  $TOKEN_ADDRESS"
echo "USDC:   $USDC_ADDRESS"
echo ""
echo "Deploy complete. Service can start."

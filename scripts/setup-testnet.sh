#!/usr/bin/env bash
set -euo pipefail

# SoroPerps Testnet Account Setup
# Creates and funds deployer + test trader accounts on Stellar testnet.
#
# Prerequisites:
#   - stellar CLI installed
#
# Usage:
#   ./scripts/setup-testnet.sh

NETWORK="testnet"

echo "=== SoroPerps Testnet Setup ==="
echo ""

# Create deployer identity
echo "--- Creating deployer identity ---"
if stellar keys show soroperps-deployer >/dev/null 2>&1; then
    echo "  Identity 'soroperps-deployer' already exists."
else
    stellar keys generate soroperps-deployer --network "$NETWORK"
    echo "  Created 'soroperps-deployer'"
fi

DEPLOYER_ADDR=$(stellar keys address soroperps-deployer)
echo "  Address: $DEPLOYER_ADDR"
echo ""

# Fund deployer via friendbot
echo "--- Funding deployer via friendbot ---"
curl -s "https://friendbot.stellar.org/?addr=${DEPLOYER_ADDR}" > /dev/null
echo "  Funded!"
echo ""

# Create test trader accounts
for i in 1 2 3; do
    IDENTITY="soroperps-trader${i}"
    echo "--- Creating ${IDENTITY} ---"
    if stellar keys show "$IDENTITY" >/dev/null 2>&1; then
        echo "  Identity '${IDENTITY}' already exists."
    else
        stellar keys generate "$IDENTITY" --network "$NETWORK"
        echo "  Created '${IDENTITY}'"
    fi

    ADDR=$(stellar keys address "$IDENTITY")
    echo "  Address: $ADDR"

    echo "  Funding via friendbot..."
    curl -s "https://friendbot.stellar.org/?addr=${ADDR}" > /dev/null
    echo "  Funded!"
    echo ""
done

echo "=== Setup Complete ==="
echo ""
echo "Identities created:"
echo "  soroperps-deployer (admin/deployer)"
echo "  soroperps-trader1"
echo "  soroperps-trader2"
echo "  soroperps-trader3"
echo ""
echo "Next step: Run ./scripts/deploy.sh to deploy contracts"

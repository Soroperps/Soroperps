#!/usr/bin/env bash
set -euo pipefail

# SoroPerps Deployment Script
# Builds, deploys, and initializes all contracts on Stellar testnet.
#
# Prerequisites:
#   - stellar CLI installed (https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli)
#   - Funded testnet account (run setup-testnet.sh first)
#
# Usage:
#   ./scripts/deploy.sh [--network testnet|futurenet]

NETWORK="${1:-testnet}"
CONFIG_FILE="deployments/${NETWORK}.json"

# Source identity (must exist from setup-testnet.sh)
IDENTITY="soroperps-deployer"

echo "=== SoroPerps Deployment ==="
echo "Network:  $NETWORK"
echo "Identity: $IDENTITY"
echo ""

# Step 1: Build all contracts
echo "--- Building contracts ---"
stellar contract build
echo "Build complete."
echo ""

# Locate WASM files
WASM_DIR="target/wasm32-unknown-unknown/release"
VAULT_WASM="${WASM_DIR}/perps_vault.wasm"
ORACLE_WASM="${WASM_DIR}/perps_oracle_adapter.wasm"
PM_WASM="${WASM_DIR}/perps_position_manager.wasm"
LE_WASM="${WASM_DIR}/perps_liquidation_engine.wasm"
FR_WASM="${WASM_DIR}/perps_funding_rate.wasm"

for wasm in "$VAULT_WASM" "$ORACLE_WASM" "$PM_WASM" "$LE_WASM" "$FR_WASM"; do
    if [ ! -f "$wasm" ]; then
        echo "ERROR: WASM not found: $wasm"
        exit 1
    fi
    size=$(wc -c < "$wasm")
    echo "  $(basename "$wasm"): ${size} bytes"
done
echo ""

# Step 2: Deploy contracts
echo "--- Deploying contracts ---"

deploy_contract() {
    local name=$1
    local wasm=$2
    echo "  Deploying ${name}..."
    local contract_id
    contract_id=$(stellar contract deploy \
        --wasm "$wasm" \
        --source "$IDENTITY" \
        --network "$NETWORK" \
        2>&1)
    echo "  ${name}: ${contract_id}"
    echo "$contract_id"
}

VAULT_ID=$(deploy_contract "vault" "$VAULT_WASM")
ORACLE_ID=$(deploy_contract "oracle-adapter" "$ORACLE_WASM")
PM_ID=$(deploy_contract "position-manager" "$PM_WASM")
LE_ID=$(deploy_contract "liquidation-engine" "$LE_WASM")
FR_ID=$(deploy_contract "funding-rate" "$FR_WASM")

echo ""
echo "--- All contracts deployed ---"
echo "  Vault:              $VAULT_ID"
echo "  Oracle Adapter:     $ORACLE_ID"
echo "  Position Manager:   $PM_ID"
echo "  Liquidation Engine: $LE_ID"
echo "  Funding Rate:       $FR_ID"
echo ""

# Step 3: Get deployer address
DEPLOYER_ADDR=$(stellar keys address "$IDENTITY" 2>&1)
echo "Deployer address: $DEPLOYER_ADDR"
echo ""

# Step 4: Initialize contracts
echo "--- Initializing contracts ---"

# USDC token on testnet (use native XLM wrapped as SAC for testing)
# In production, use the actual USDC token contract
USDC_TOKEN="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"  # Testnet USDC

echo "  Initializing vault..."
stellar contract invoke \
    --id "$VAULT_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    initialize \
    --admin "$DEPLOYER_ADDR" \
    --usdc_token "$USDC_TOKEN" \
    --max_utilization_bps 8000

echo "  Initializing oracle adapter..."
stellar contract invoke \
    --id "$ORACLE_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    initialize \
    --admin "$DEPLOYER_ADDR" \
    --staleness_threshold 300

echo "  Initializing position manager..."
stellar contract invoke \
    --id "$PM_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    initialize \
    --admin "$DEPLOYER_ADDR" \
    --vault "$VAULT_ID" \
    --oracle "$ORACLE_ID" \
    --usdc_token "$USDC_TOKEN" \
    --open_fee_bps 10 \
    --close_fee_bps 10 \
    --max_leverage 30 \
    --min_collateral 10000000

echo "  Initializing liquidation engine..."
stellar contract invoke \
    --id "$LE_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    initialize \
    --admin "$DEPLOYER_ADDR" \
    --position_manager "$PM_ID" \
    --oracle "$ORACLE_ID" \
    --maintenance_margin_bps 500 \
    --liquidation_penalty_bps 250 \
    --keeper_reward_bps 50

echo "  Initializing funding rate..."
stellar contract invoke \
    --id "$FR_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    initialize \
    --admin "$DEPLOYER_ADDR" \
    --position_manager "$PM_ID" \
    --funding_interval 3600 \
    --max_funding_rate_bps 10 \
    --funding_spread_bps 5

echo ""
echo "--- Wiring contracts ---"

echo "  Setting position manager on vault..."
stellar contract invoke \
    --id "$VAULT_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    set_position_manager \
    --admin "$DEPLOYER_ADDR" \
    --pm_address "$PM_ID"

echo "  Setting liquidation engine on position manager..."
stellar contract invoke \
    --id "$PM_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    set_liquidation_engine \
    --admin "$DEPLOYER_ADDR" \
    --le_address "$LE_ID"

echo "  Setting funding address on position manager..."
stellar contract invoke \
    --id "$PM_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    set_funding_address \
    --admin "$DEPLOYER_ADDR" \
    --funding "$FR_ID"

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "Update your keeper config with these contract IDs:"
echo "{"
echo "  \"vault_contract_id\": \"$VAULT_ID\","
echo "  \"position_manager_contract_id\": \"$PM_ID\","
echo "  \"oracle_adapter_contract_id\": \"$ORACLE_ID\","
echo "  \"liquidation_engine_contract_id\": \"$LE_ID\","
echo "  \"funding_rate_contract_id\": \"$FR_ID\""
echo "}"

#!/usr/bin/env bash
# -----------------------------------------------------------------------
# deploy-testnet.sh — Deploy SLA Calculator contract to Stellar testnet
#
# Usage:
#   ./scripts/deploy-testnet.sh [OPTIONS]
#
# Options:
#   --network TESTNET|MAINNET    Target network (default: TESTNET)
#   --admin ADDRESS              Admin address (required)
#   --operator ADDRESS           Operator address (optional, defaults to admin)
#   --dry-run                    Print actions without executing
#   --help                       Show this help message
#
# Prerequisites:
#   - stellar-cli installed and in PATH
#   - Freighter wallet configured (for interactive signing)
#   - Sufficient XLM balance for contract deployment
# -----------------------------------------------------------------------

set -euo pipefail

# Defaults
NETWORK="TESTNET"
ADMIN=""
OPERATOR=""
DRY_RUN=false
CONTRACT_NAME="sla_calculator"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Deploy SLA Calculator contract to Stellar testnet"
    echo ""
    echo "Options:"
    echo "  --network TESTNET|MAINNET    Target network (default: TESTNET)"
    echo "  --admin ADDRESS              Admin address (required)"
    echo "  --operator ADDRESS           Operator address (optional, defaults to admin)"
    echo "  --dry-run                    Print actions without executing"
    echo "  --help                       Show this help message"
    exit 0
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --network)
            NETWORK="$2"
            shift 2
            ;;
        --admin)
            ADMIN="$2"
            shift 2
            ;;
        --operator)
            OPERATOR="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --help)
            usage
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            ;;
    esac
done

# Validate inputs
if [[ -z "$ADMIN" ]]; then
    log_error "Admin address is required (--admin)"
    exit 1
fi

if [[ -z "$OPERATOR" ]]; then
    OPERATOR="$ADMIN"
    log_warn "Operator not specified, using admin address: $ADMIN"
fi

if [[ "$NETWORK" != "TESTNET" && "$NETWORK" != "MAINNET" ]]; then
    log_error "Invalid network: $NETWORK (must be TESTNET or MAINNET)"
    exit 1
fi

# Determine network passphrase
if [[ "$NETWORK" == "TESTNET" ]]; then
    NETWORK_PASSPHRASE="Testnet ; SDF Network ; September 2015"
else
    NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
fi

log_info "Deployment configuration:"
log_info "  Network:     $NETWORK"
log_info "  Admin:       $ADMIN"
log_info "  Operator:    $OPERATOR"
log_info "  Contract:    $CONTRACT_NAME"
echo ""

# Step 1: Build the contract
log_info "Step 1: Building contract..."
if [[ "$DRY_RUN" == "false" ]]; then
    stellar contract build --package sla_calculator
    log_success "Contract built successfully"
else
    log_info "  [DRY RUN] Would build: stellar contract build --package sla_calculator"
fi

# Step 2: Optimize the WASM
log_info "Step 2: Optimizing WASM..."
WASM_PATH="target/wasm32-unknown-unknown/release/sla_calculator.wasm"
if [[ "$DRY_RUN" == "false" ]]; then
    if [[ ! -f "$WASM_PATH" ]]; then
        log_error "WASM not found at $WASM_PATH"
        exit 1
    fi
    stellar contract optimize --wasm "$WASM_PATH"
    OPTIMIZED_PATH="${WASM_PATH%.wasm}.optimized.wasm"
    log_success "WASM optimized: $OPTIMIZED_PATH"
else
    log_info "  [DRY RUN] Would optimize: $WASM_PATH"
    OPTIMIZED_PATH="${WASM_PATH%.wasm}.optimized.wasm"
fi

# Step 3: Deploy the contract
log_info "Step 3: Deploying contract..."
if [[ "$DRY_RUN" == "false" ]]; then
    CONTRACT_ID=$(stellar contract deploy \
        --wasm "$OPTIMIZED_PATH" \
        --network "$NETWORK" \
        --source auto \
        --rpc-url "https://soroban-${NETWORK,,}.stellar.org" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        2>&1 | tee /dev/stderr | grep -oP 'Contract ID: \K[a-zA-Z0-9]+')
    log_success "Contract deployed: $CONTRACT_ID"
else
    CONTRACT_ID="C_PLACEHOLDER_1234567890ABCDEF"
    log_info "  [DRY RUN] Would deploy: $OPTIMIZED_PATH"
    log_info "  [DRY RUN] Would get contract ID"
fi

# Step 4: Initialize the contract
log_info "Step 4: Initializing contract..."
if [[ "$DRY_RUN" == "false" ]]; then
    stellar contract invoke \
        --id "$CONTRACT_ID" \
        --fn initialize \
        --arg "$ADMIN" \
        --arg "$OPERATOR" \
        --network "$NETWORK" \
        --source auto \
        --rpc-url "https://soroban-${NETWORK,,}.stellar.org" \
        --network-passphrase "$NETWORK_PASSPHRASE"
    log_success "Contract initialized"
else
    log_info "  [DRY RUN] Would invoke: initialize($ADMIN, $OPERATOR)"
fi

# Step 5: Verify deployment
log_info "Step 5: Verifying deployment..."
if [[ "$DRY_RUN" == "false" ]]; then
    VERSION_INFO=$(stellar contract invoke \
        --id "$CONTRACT_ID" \
        --fn get_version_info \
        --network "$NETWORK" \
        --rpc-url "https://soroban-${NETWORK,,}.stellar.org" \
        --network-passphrase "$NETWORK_PASSPHRASE" \
        2>&1)
    log_success "Version info: $VERSION_INFO"
else
    log_info "  [DRY RUN] Would verify: get_version_info()"
fi

# Summary
echo ""
log_success "Deployment complete!"
log_info "Contract ID: $CONTRACT_ID"
log_info "Network:     $NETWORK"
log_info "Admin:       $ADMIN"
log_info "Operator:    $OPERATOR"
echo ""
log_info "Next steps:"
log_info "  1. Save the contract ID for backend configuration"
log_info "  2. Update backend .env with CONTRACT_ID=$CONTRACT_ID"
log_info "  3. Run smoke tests against the deployed contract"

#!/usr/bin/env bash
# -----------------------------------------------------------------------
# deploy-multi-network.sh — Deploy SLA Calculator to multiple networks
#
# Usage:
#   ./scripts/deploy-multi-network.sh [OPTIONS]
#
# Options:
#   --networks TESTNET,MAINNET   Comma-separated network list
#   --admin ADDRESS              Admin address (required)
#   --operator ADDRESS           Operator address (optional)
#   --config FILE                Deploy config file (optional)
#   --dry-run                    Print actions without executing
#   --help                       Show this help message
#
# Prerequisites:
#   - stellar-cli installed and in PATH
#   - Freighter wallet configured (for interactive signing)
#   - Sufficient XLM balances on target networks
# -----------------------------------------------------------------------

set -euo pipefail

# Defaults
NETWORKS="TESTNET"
ADMIN=""
OPERATOR=""
CONFIG_FILE=""
DRY_RUN=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Network configurations
declare -A NETWORK_PASSPHRASES=(
    ["TESTNET"]="Testnet ; SDF Network ; September 2015"
    ["MAINNET"]="Public Global Stellar Network ; September 2015"
)

declare -A NETWORK_RPC_URLS=(
    ["TESTNET"]="https://soroban-testnet.stellar.org"
    ["MAINNET"]="https://soroban-mainnet.stellar.org"
)

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Deploy SLA Calculator to multiple Stellar networks"
    echo ""
    echo "Options:"
    echo "  --networks TESTNET,MAINNET   Comma-separated network list (default: TESTNET)"
    echo "  --admin ADDRESS              Admin address (required)"
    echo "  --operator ADDRESS           Operator address (optional, defaults to admin)"
    echo "  --config FILE                Deploy config file (JSON)"
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

log_network() {
    echo -e "${CYAN}[NETWORK]${NC} $1"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --networks)
            NETWORKS="$2"
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
        --config)
            CONFIG_FILE="$2"
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

# Load config file if provided
if [[ -n "$CONFIG_FILE" && -f "$CONFIG_FILE" ]]; then
    log_info "Loading config from $CONFIG_FILE"
    ADMIN=$(jq -r '.admin // empty' "$CONFIG_FILE")
    OPERATOR=$(jq -r '.operator // empty' "$CONFIG_FILE")
    NETWORKS=$(jq -r '.networks // "TESTNET"' "$CONFIG_FILE")
fi

# Validate inputs
if [[ -z "$ADMIN" ]]; then
    log_error "Admin address is required (--admin)"
    exit 1
fi

if [[ -z "$OPERATOR" ]]; then
    OPERATOR="$ADMIN"
    log_warn "Operator not specified, using admin address: $ADMIN"
fi

# Convert networks string to array
IFS=',' read -ra NETWORK_ARRAY <<< "$NETWORKS"

log_info "Multi-network deployment configuration:"
log_info "  Networks:  ${NETWORK_ARRAY[*]}"
log_info "  Admin:     $ADMIN"
log_info "  Operator:  $OPERATOR"
echo ""

# Track deployment results
declare -A DEPLOYMENT_RESULTS=()

# Deploy to each network
for NETWORK in "${NETWORK_ARRAY[@]}"; do
    NETWORK=$(echo "$NETWORK" | xargs) # Trim whitespace
    
    log_network "Deploying to $NETWORK..."
    
    if [[ -z "${NETWORK_PASSPHRASES[$NETWORK]+x}" ]]; then
        log_error "Unknown network: $NETWORK"
        DEPLOYMENT_RESULTS[$NETWORK]="FAILED:Unknown network"
        continue
    fi
    
    PASSPHRASE="${NETWORK_PASSPHRASES[$NETWORK]}"
    RPC_URL="${NETWORK_RPC_URLS[$NETWORK]}"
    
    if [[ "$DRY_RUN" == "false" ]]; then
        # Build
        log_info "  Building contract..."
        stellar contract build --package sla_calculator
        
        # Optimize
        WASM_PATH="target/wasm32-unknown-unknown/release/sla_calculator.wasm"
        stellar contract optimize --wasm "$WASM_PATH"
        OPTIMIZED_PATH="${WASM_PATH%.wasm}.optimized.wasm"
        
        # Deploy
        log_info "  Deploying..."
        CONTRACT_ID=$(stellar contract deploy \
            --wasm "$OPTIMIZED_PATH" \
            --network "$NETWORK" \
            --source auto \
            --rpc-url "$RPC_URL" \
            --network-passphrase "$PASSPHRASE" \
            2>&1 | tee /dev/stderr | grep -oP 'Contract ID: \K[a-zA-Z0-9]+')
        
        # Initialize
        log_info "  Initializing..."
        stellar contract invoke \
            --id "$CONTRACT_ID" \
            --fn initialize \
            --arg "$ADMIN" \
            --arg "$OPERATOR" \
            --network "$NETWORK" \
            --source auto \
            --rpc-url "$RPC_URL" \
            --network-passphrase "$PASSPHRASE"
        
        DEPLOYMENT_RESULTS[$NETWORK]="SUCCESS:$CONTRACT_ID"
        log_success "  Deployed: $CONTRACT_ID"
    else
        log_info "  [DRY RUN] Would deploy to $NETWORK"
        DEPLOYMENT_RESULTS[$NETWORK]="DRY_RUN:PENDING"
    fi
    
    echo ""
done

# Summary
log_info "Deployment Summary:"
echo "==================="
for NETWORK in "${NETWORK_ARRAY[@]}"; do
    NETWORK=$(echo "$NETWORK" | xargs)
    RESULT="${DEPLOYMENT_RESULTS[$NETWORK]:-NOT_STARTED}"
    if [[ "$RESULT" == SUCCESS:* ]]; then
        CONTRACT_ID="${RESULT#SUCCESS:}"
        echo -e "${GREEN}✓${NC} $NETWORK: $CONTRACT_ID"
    elif [[ "$RESULT" == DRY_RUN:* ]]; then
        echo -e "${YELLOW}○${NC} $NETWORK: DRY RUN"
    else
        echo -e "${RED}✗${NC} $NETWORK: $RESULT"
    fi
done

echo ""
log_info "Next steps:"
log_info "  1. Save contract IDs for each network"
log_info "  2. Update backend configs with network-specific contract IDs"
log_info "  3. Run smoke tests on each deployed contract"

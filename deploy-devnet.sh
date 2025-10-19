#!/bin/bash

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CHAIN_ID="cosmoshub-devnet-1"
NODE="http://localhost:16657"
KEYRING_BACKEND="test"
WALLET_NAME="devnet-deployer"
GAS_PRICES="0.025uatom"
GAS="auto"
GAS_ADJUSTMENT="1.3"

# Contract paths
LSM_STAKING_WASM="artifacts/lsm_staking.wasm"
PROPOSAL_LOCKER_WASM="artifacts/proposal_option_locker.wasm"

# Helper functions
print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}→${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_header() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
}

# Check if gaiad is available
check_gaiad() {
    if ! command -v gaiad &> /dev/null; then
        print_error "gaiad command not found. Please install Gaia."
        exit 1
    fi
    print_success "gaiad found: $(gaiad version)"
}

# Check if wallet exists
check_wallet() {
    print_info "Checking if wallet '$WALLET_NAME' exists..."

    if ! gaiad keys show "$WALLET_NAME" --keyring-backend "$KEYRING_BACKEND" &> /dev/null; then
        print_error "Wallet '$WALLET_NAME' not found in keyring"
        print_info "Creating wallet '$WALLET_NAME'..."
        gaiad keys add "$WALLET_NAME" --keyring-backend "$KEYRING_BACKEND"

        if [ $? -ne 0 ]; then
            print_error "Failed to create wallet"
            exit 1
        fi

        print_warning "Wallet created! Please fund it before continuing."
        print_info "Wallet address: $(gaiad keys show $WALLET_NAME --keyring-backend $KEYRING_BACKEND -a)"
        read -p "Press enter when wallet is funded..."
    else
        print_success "Wallet found: $(gaiad keys show $WALLET_NAME --keyring-backend $KEYRING_BACKEND -a)"
    fi
}

# Check wallet balance
check_balance() {
    local address=$(gaiad keys show $WALLET_NAME --keyring-backend $KEYRING_BACKEND -a)
    print_info "Checking balance for $address..."

    local balance=$(gaiad query bank balances "$address" --node "$NODE" --output json 2>/dev/null | jq -r '.balances[] | select(.denom=="uatom") | .amount')

    if [ -z "$balance" ] || [ "$balance" == "0" ]; then
        print_error "Wallet has no balance. Please fund it first."
        exit 1
    fi

    print_success "Balance: $balance uatom"
}

# Check if WASM files exist
check_wasm_files() {
    print_info "Checking WASM files..."

    if [ ! -f "$LSM_STAKING_WASM" ]; then
        print_error "LSM Staking WASM not found: $LSM_STAKING_WASM"
        print_info "Run 'make optimize' first to build optimized WASM files"
        exit 1
    fi

    if [ ! -f "$PROPOSAL_LOCKER_WASM" ]; then
        print_error "Proposal Locker WASM not found: $PROPOSAL_LOCKER_WASM"
        print_info "Run 'make optimize' first to build optimized WASM files"
        exit 1
    fi

    print_success "WASM files found:"
    ls -lh "$LSM_STAKING_WASM" | awk '{print "  - " $9 " (" $5 ")"}'
    ls -lh "$PROPOSAL_LOCKER_WASM" | awk '{print "  - " $9 " (" $5 ")"}'
}

# Store contract code
store_contract() {
    local wasm_file=$1
    local contract_name=$2

    print_info "Storing $contract_name contract..."

    # Execute transaction and capture output
    local tx_output=$(gaiad tx wasm store "$wasm_file" \
        --from "$WALLET_NAME" \
        --keyring-backend "$KEYRING_BACKEND" \
        --node "$NODE" \
        --chain-id "$CHAIN_ID" \
        --gas "$GAS" \
        --gas-adjustment "$GAS_ADJUSTMENT" \
        --gas-prices "$GAS_PRICES" \
        --broadcast-mode sync \
        --yes \
        --output json 2>&1)

    # Check if output is valid JSON
    if ! echo "$tx_output" | jq empty 2>/dev/null; then
        print_error "Transaction failed or returned invalid JSON"
        echo "Output: $tx_output" >&2
        return 1
    fi

    local tx_hash=$(echo "$tx_output" | jq -r '.txhash')

    if [ -z "$tx_hash" ] || [ "$tx_hash" == "null" ]; then
        print_error "Failed to get transaction hash for $contract_name contract"
        echo "Output: $tx_output" >&2
        return 1
    fi

    print_info "Transaction hash: $tx_hash"
    print_info "Waiting for transaction to be included in a block..."
    sleep 6

    # Query transaction to get code_id
    local tx_result=$(gaiad query tx "$tx_hash" --node "$NODE" --output json 2>&1)

    if ! echo "$tx_result" | jq empty 2>/dev/null; then
        print_error "Failed to query transaction"
        echo "Output: $tx_result" >&2
        return 1
    fi

    local code_id=$(echo "$tx_result" | jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value' | tr -d '"')

    if [ -z "$code_id" ] || [ "$code_id" == "null" ]; then
        print_error "Failed to get code_id for $contract_name"
        echo "Transaction result: $tx_result" >&2
        return 1
    fi

    print_success "$contract_name stored with code_id: $code_id"
    echo "$code_id"
}

# Instantiate contract
instantiate_contract() {
    local code_id=$1
    local init_msg=$2
    local label=$3
    local contract_name=$4

    print_info "Instantiating $contract_name contract..."
    print_info "Init message: $init_msg"

    # Execute transaction and capture output
    local tx_output=$(gaiad tx wasm instantiate "$code_id" "$init_msg" \
        --from "$WALLET_NAME" \
        --keyring-backend "$KEYRING_BACKEND" \
        --node "$NODE" \
        --chain-id "$CHAIN_ID" \
        --label "$label" \
        --gas "$GAS" \
        --gas-adjustment "$GAS_ADJUSTMENT" \
        --gas-prices "$GAS_PRICES" \
        --no-admin \
        --broadcast-mode sync \
        --yes \
        --output json 2>&1)

    # Check if output is valid JSON
    if ! echo "$tx_output" | jq empty 2>/dev/null; then
        print_error "Transaction failed or returned invalid JSON"
        echo "Output: $tx_output" >&2
        return 1
    fi

    local tx_hash=$(echo "$tx_output" | jq -r '.txhash')

    if [ -z "$tx_hash" ] || [ "$tx_hash" == "null" ]; then
        print_error "Failed to get transaction hash for $contract_name"
        echo "Output: $tx_output" >&2
        return 1
    fi

    print_info "Transaction hash: $tx_hash"
    print_info "Waiting for transaction to be included in a block..."
    sleep 6

    # Query transaction to get contract address
    local tx_result=$(gaiad query tx "$tx_hash" --node "$NODE" --output json 2>&1)

    if ! echo "$tx_result" | jq empty 2>/dev/null; then
        print_error "Failed to query transaction"
        echo "Output: $tx_result" >&2
        return 1
    fi

    local contract_address=$(echo "$tx_result" | jq -r '.events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value' | tr -d '"')

    if [ -z "$contract_address" ] || [ "$contract_address" == "null" ]; then
        print_error "Failed to get contract address for $contract_name"
        echo "Transaction result: $tx_result" >&2
        return 1
    fi

    print_success "$contract_name instantiated at: $contract_address"
    echo "$contract_address"
}

# Get validator address
get_validator() {
    print_info "Querying validators..."

    local validator=$(gaiad query staking validators --node "$NODE" --output json 2>/dev/null | \
        jq -r '.validators[0].operator_address')

    if [ -z "$validator" ] || [ "$validator" == "null" ]; then
        print_error "No validators found on the network"
        print_warning "Using default validator address for testing"
        echo "cosmosvaloper1..."
    else
        print_success "Using validator: $validator"
        echo "$validator"
    fi
}

# Main deployment flow
main() {
    print_header "CosmWasm Deployment Script - DevNet"

    # Pre-flight checks
    check_gaiad
    check_wallet
    check_balance
    check_wasm_files

    # Get deployer address
    DEPLOYER_ADDRESS=$(gaiad keys show $WALLET_NAME --keyring-backend $KEYRING_BACKEND -a)
    print_info "Deployer address: $DEPLOYER_ADDRESS"

    # Get validator
    VALIDATOR=$(get_validator)

    # Step 1: Store Proposal Locker contract
    print_header "Step 1: Store Proposal Locker Contract"
    LOCKER_CODE_ID=$(store_contract "$PROPOSAL_LOCKER_WASM" "Proposal Locker")
    if [ -z "$LOCKER_CODE_ID" ]; then
        print_error "Failed to store Proposal Locker contract"
        exit 1
    fi

    # Step 2: Store LSM Staking contract
    print_header "Step 2: Store LSM Staking Contract"
    LSM_CODE_ID=$(store_contract "$LSM_STAKING_WASM" "LSM Staking")
    if [ -z "$LSM_CODE_ID" ]; then
        print_error "Failed to store LSM Staking contract"
        exit 1
    fi

    # Step 3: Instantiate LSM Staking contract with Locker code_id
    print_header "Step 3: Instantiate LSM Staking Contract"

    # Create instantiate message
    LSM_INIT_MSG=$(cat <<EOF
{
  "owner": "$DEPLOYER_ADDRESS",
  "staking_denom": "uatom",
  "validator": "$VALIDATOR",
  "max_cap": null,
  "locker_code_id": $LOCKER_CODE_ID
}
EOF
)

    LSM_CONTRACT_ADDRESS=$(instantiate_contract "$LSM_CODE_ID" "$LSM_INIT_MSG" "LSM Staking Contract" "LSM Staking")
    if [ -z "$LSM_CONTRACT_ADDRESS" ]; then
        print_error "Failed to instantiate LSM Staking contract"
        exit 1
    fi

    # Summary
    print_header "Deployment Summary"
    echo -e "${GREEN}Deployment successful!${NC}"
    echo ""
    echo "Contract Code IDs:"
    echo "  - Proposal Locker Code ID: $LOCKER_CODE_ID"
    echo "  - LSM Staking Code ID: $LSM_CODE_ID"
    echo ""
    echo "Contract Addresses:"
    echo "  - LSM Staking Contract: $LSM_CONTRACT_ADDRESS"
    echo ""
    echo "Configuration:"
    echo "  - Owner: $DEPLOYER_ADDRESS"
    echo "  - Validator: $VALIDATOR"
    echo "  - Staking Denom: uatom"
    echo "  - Max Cap: null (unlimited)"
    echo ""

    # Save deployment info to file
    DEPLOYMENT_FILE="deployment-info.json"
    cat > "$DEPLOYMENT_FILE" <<EOF
{
  "network": "devnet",
  "chain_id": "$CHAIN_ID",
  "node": "$NODE",
  "deployer": "$DEPLOYER_ADDRESS",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "contracts": {
    "proposal_locker": {
      "code_id": $LOCKER_CODE_ID,
      "wasm_file": "$PROPOSAL_LOCKER_WASM"
    },
    "lsm_staking": {
      "code_id": $LSM_CODE_ID,
      "wasm_file": "$LSM_STAKING_WASM",
      "address": "$LSM_CONTRACT_ADDRESS",
      "instantiate_msg": $LSM_INIT_MSG
    }
  },
  "validator": "$VALIDATOR"
}
EOF

    print_success "Deployment info saved to: $DEPLOYMENT_FILE"
    echo ""
    print_info "You can now interact with the LSM Staking contract at: $LSM_CONTRACT_ADDRESS"
}

# Run main function
main

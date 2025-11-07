# LSM Staking & Voting Power Rental Contracts

CosmWasm smart contracts for LSM (Liquid Staking Module) staking and governance voting power rental on Cosmos Hub (Gaia).

## ⚠️ Proof of Concept

**This project is a proof of concept and has NOT been audited. DO NOT use in production with real funds. Use at your own risk.**

## 📋 Quick Start

```bash
# 1. Build contracts
make build

# 2. Deploy to devnet
make deploy-devnet
```

All dependencies and configuration are included in the repository - no manual setup needed!

## Overview

This project implements a voting power rental system for Cosmos Hub governance. It consists of two types of smart contracts:

### 1. LSM-Staking Contract (Main Contract)
The main contract that manages atom deposits and voting power rental.

### 2. Proposal-Option-Locker Contracts
Secondary contracts dynamically instantiated for each voting option of a governance proposal.

## How It Works

### Phase 1: Regular Deposit and Staking

1. **LSM Share Deposit**: Users deposit LSM shares from the validator specified during instantiation
2. **Automatic Redeem**: Upon receipt, LSM shares are automatically redeemed
3. **Atom Custody**: The contract maintains custody of staked atoms at all times
4. **Reward Management**: Users can claim their rewards or withdraw their LSM shares at any time

### Phase 2: Opening a Governance Proposal

1. **Admin Opens**: An admin can open voting power rental for a specific proposal
2. **Option-Locker Instantiation**: The system instantiates as many "proposal-option-locker" contracts as there are possible options:
   - YES locker → votes YES
   - NO locker → votes NO
   - NO_WITH_VETO locker → votes NO_WITH_VETO
   - ABSTAIN locker → votes ABSTAIN
3. **Automatic Voting**: Each option-locker contract automatically votes for its respective option upon instantiation
4. **Operation Lock**: While a proposal is active, users can no longer deposit or withdraw

### Phase 3: Voting Power Rental

1. **User Rental**: A user can rent atom voting power by choosing an option to support
2. **Tokenization**: The chosen atom amount is tokenized
3. **Transfer to Option-Locker**: The tokens are transferred to the smart contract of the chosen option
4. **Redeem and Vote**: The option-locker contract redeems these shares, which increases the vote allocated to this option

### Phase 4: End of Proposal

1. **Option-Locker Destruction**: When the proposal ends, the option-locker contracts can be destroyed
2. **Reward Recovery**: Each option-locker recovers accumulated staking rewards
3. **Send to LSM Contract**: The rewards are sent to the main LSM contract
4. **Tokenization and Redeem**: The option-lockers tokenize all their staked atoms and send them to the LSM contract which redeems them
5. **Unlock**: The LSM contract unlocks deposit and withdraw operations

## Features

- **LSM Share Deposits**: Accepts LSM shares from the validator specified at instantiation, automatically redeems them
- **Validator Verification**: Validates that the LSM share validator exists on-chain before accepting
- **Cumulative Reward Index Algorithm**: Fair and gas-efficient reward distribution
- **Reward Claiming**: Users can claim their accumulated staking rewards
- **Staking Withdrawal**: Users can withdraw (unstake) their tokens, they receive LSM shares
- **Voting Power Rental**: Rent voting power for governance proposals
- **Dynamic Proposal Option Lockers**: Dynamic instantiation of contracts for each voting option
- **Automatic Voting**: Automatic voting for each option upon locker instantiation
- **Operation Lock During Proposals**: Deposits/withdrawals are blocked during active proposals
- **Admin Functions**: The contract owner can update configuration and manage proposals

## Architecture

The project is organized as a Cargo workspace with two contracts:

```
rent_voting_power/
├── contracts/
│   ├── lsm-staking/              # Main contract
│   │   ├── src/
│   │   │   ├── contract.rs       # Contract logic (instantiate, execute, query)
│   │   │   ├── error.rs          # Error types
│   │   │   ├── state.rs          # State storage definitions
│   │   │   └── lib.rs            # Library exports
│   │   └── Cargo.toml
│   └── proposal-option-locker/   # Vote option contract
│       ├── src/
│       │   ├── contract.rs       # Option-locker contract logic
│       │   ├── error.rs          # Error types
│       │   ├── state.rs          # State storage definitions
│       │   └── lib.rs            # Library exports
│       └── Cargo.toml
└── packages/
    └── lsm-types/                # Shared types and messages
        ├── src/
        │   └── lib.rs            # Message and state types
        └── Cargo.toml
```

### Data Flow

```
User
    ↓ (deposits LSM shares)
LSM-Staking Contract
    ↓ (redeem → staked atoms)
Atom Custody
    ↓ (proposal opened)
Instantiation of 4 Proposal-Option-Locker Contracts
    ├─ YES Option Locker (votes YES)
    ├─ NO Option Locker (votes NO)
    ├─ NO_WITH_VETO Option Locker (votes NO_WITH_VETO)
    └─ ABSTAIN Option Locker (votes ABSTAIN)
    ↓ (user rents voting power)
Atom Tokenization → Transfer to chosen Option-Locker
    ↓ (redeem → increases vote)
Vote allocated to option
    ↓ (proposal ends)
Destruction of Option-Lockers
    ↓ (reward recovery + tokenization)
LSM-Staking Contract (redeem and unlock)
```

## Cumulative Reward Index Algorithm

The contract uses a cumulative reward index algorithm for efficient reward distribution:

### How It Works

1. **Global Reward Index**: Tracks cumulative rewards per staked token

   ```
   global_index = Σ(rewards_deposited / total_staked)
   ```

2. **User Reward Index**: Each user has their own index snapshot

   - Updated when user stakes/unstakes or claims rewards

3. **Pending Rewards Calculation**:
   ```
   pending_rewards = staked_amount × (global_index - user_index) + stored_pending
   ```

### Benefits

- **Gas Efficient**: O(1) complexity for reward distribution
- **Fair Distribution**: Proportional to stake amount and duration
- **No Iteration**: Doesn't require iterating through all stakers
- **Accurate**: Handles any number of stakers without precision loss

## Contract Messages

### LSM-Staking Contract

#### InstantiateMsg

```rust
{
  "staking_denom": "uatom",        // Base staking denom
  "validator": "cosmosvaloper1...", // Validator for LSM shares
  "owner": "cosmos1..."             // Contract admin
}
```

#### ExecuteMsg

##### DepositLsmShares

Deposit LSM shares to be redeemed and staked:

```rust
{
  "deposit_lsm_shares": {}
}
// Send EXACTLY ONE LSM share token as funds
// LSM denom format: {validator_address}/{record_id}
// Example: cosmosvaloper1abc.../123
```

The contract will:

1. Verify exactly one token is sent
2. Parse and validate the LSM denom format (validator/record_id)
3. Verify the validator address is valid
4. Verify the record_id is a valid number
5. Redeem the LSM shares and stake them
6. **Blocked if a proposal is active**

##### ClaimRewards

Claim accumulated staking rewards:

```rust
{
  "claim_rewards": {}
}
```

##### Withdraw

Withdraw (unstake) tokens from a validator:

```rust
{
  "withdraw": {
    "amount": "1000000",
    "validator": "cosmosvaloper1..."
  }
}
```

**Blocked if a proposal is active**

##### OpenProposal

Open a governance proposal and instantiate option-lockers (admin only):

```rust
{
  "open_proposal": {
    "proposal_id": 123,
    "option_locker_code_id": 456  // Code ID of the proposal-option-locker contract
  }
}
```

This action will:
1. Instantiate 4 proposal-option-locker contracts (YES, NO, NO_WITH_VETO, ABSTAIN)
2. Each contract automatically votes for its option
3. Block deposit and withdraw operations

##### RentVotingPower

Rent voting power to support an option (user):

```rust
{
  "rent_voting_power": {
    "amount": "1000000",
    "option": "Yes"  // "Yes" | "No" | "NoWithVeto" | "Abstain"
  }
}
```

This action will:
1. Tokenize the specified atom amount
2. Transfer the tokens to the corresponding option-locker contract
3. The option-locker redeems these shares, increasing the vote for this option

##### CloseProposal

Close a proposal and destroy the option-lockers (admin only):

```rust
{
  "close_proposal": {}
}
```

This action will:
1. Destroy all option-locker contracts
2. Recover staking rewards from each option-locker
3. Recover all staked atoms (tokenized then redeemed)
4. Unlock deposit and withdraw operations

##### UpdateConfig

Update contract configuration (owner only):

```rust
{
  "update_config": {
    "owner": "cosmos1..."  // Optional
  }
}
```

### Proposal-Option-Locker Contract

#### InstantiateMsg

```rust
{
  "lsm_staking_contract": "cosmos1...",  // Address of the main LSM contract
  "proposal_id": 123,                     // Proposal ID
  "vote_option": "Yes",                   // "Yes" | "No" | "NoWithVeto" | "Abstain"
  "validator": "cosmosvaloper1..."        // Validator to use
}
```

The contract automatically votes for the specified option upon instantiation.

#### ExecuteMsg

##### ReceiveTokenizedShares

Receive tokenized shares from the main LSM contract:

```rust
{
  "receive_tokenized_shares": {}
}
// Shares are automatically redeemed to increase the vote
```

##### Destroy

Destroy the contract and return all assets to the LSM contract (admin only):

```rust
{
  "destroy": {}
}
```

This action will:
1. Claim all staking rewards
2. Tokenize all staked atoms
3. Send rewards and tokenized tokens to the main LSM contract

### QueryMsg

#### Config

Get contract configuration:

```rust
{
  "config": {}
}
```

#### StakerInfo

Get staker information and pending rewards:

```rust
{
  "staker_info": {
    "address": "cosmos1..."
  }
}
```

#### TotalStaked

Get total amount staked in the contract:

```rust
{
  "total_staked": {}
}
```

#### RewardIndex

Get current global reward index:

```rust
{
  "reward_index": {}
}
```

#### Stakers

List all stakers with pagination:

```rust
{
  "stakers": {
    "start_after": "cosmos1...",  // Optional
    "limit": 10                    // Optional, default: 10, max: 30
  }
}
```

## Building the Contract

### Prerequisites

- Rust 1.70+
- `wasm32-unknown-unknown` target

### Build Commands

```bash
# Build the contract
cargo build

# Build optimized WASM
cargo build --release --target wasm32-unknown-unknown

# Run tests
cargo test

# Generate schema
cargo schema
```

### Optimized Build

For production deployment, use `rust-optimizer`:

```bash
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.16.0
```

This will create an optimized `.wasm` file in the `artifacts/` directory.

## Usage Example

### 1. Instantiate LSM-Staking Contract

```bash
# Store the LSM staking contract
RES=$(gaiad tx wasm store artifacts/lsm_staking.wasm \
  --from wallet --gas auto --gas-adjustment 1.3 -y)

# Get code ID
LSM_CODE_ID=$(echo $RES | jq -r '.logs[0].events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')

# Instantiate
INIT_MSG='{
  "staking_denom": "uatom",
  "validator": "cosmosvaloper1...",
  "owner": "cosmos1..."
}'

gaiad tx wasm instantiate $LSM_CODE_ID "$INIT_MSG" \
  --from wallet --label "LSM Staking" --gas auto --gas-adjustment 1.3 -y
```

### 2. Store Proposal-Option-Locker Contract

```bash
# Store the proposal option locker contract
RES=$(gaiad tx wasm store artifacts/proposal_option_locker.wasm \
  --from wallet --gas auto --gas-adjustment 1.3 -y)

# Get code ID (will be used when opening proposals)
OPTION_LOCKER_CODE_ID=$(echo $RES | jq -r '.logs[0].events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')
```

### 3. Deposit LSM Shares (Phase 1)

```bash
LSM_CONTRACT="cosmos1..."
LSM_DENOM="cosmosvaloper1abc123def456/789"

gaiad tx wasm execute $LSM_CONTRACT \
  '{"deposit_lsm_shares":{}}' \
  --amount 1000000${LSM_DENOM} \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 4. Claim Rewards (Phase 1)

```bash
gaiad tx wasm execute $LSM_CONTRACT \
  '{"claim_rewards":{}}' \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 5. Withdraw (Phase 1)

```bash
gaiad tx wasm execute $LSM_CONTRACT \
  '{
    "withdraw": {
      "amount": "500000",
      "validator": "cosmosvaloper1..."
    }
  }' \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 6. Open Proposal (Phase 2 - Admin)

```bash
gaiad tx wasm execute $LSM_CONTRACT \
  '{
    "open_proposal": {
      "proposal_id": 123,
      "option_locker_code_id": '$OPTION_LOCKER_CODE_ID'
    }
  }' \
  --from admin_wallet --gas auto --gas-adjustment 1.3 -y
```

This command will instantiate 4 option-locker contracts and block deposits/withdrawals.

### 7. Rent Voting Power (Phase 3 - User)

```bash
gaiad tx wasm execute $LSM_CONTRACT \
  '{
    "rent_voting_power": {
      "amount": "1000000",
      "option": "Yes"
    }
  }' \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 8. Close Proposal (Phase 4 - Admin)

```bash
gaiad tx wasm execute $LSM_CONTRACT \
  '{"close_proposal":{}}' \
  --from admin_wallet --gas auto --gas-adjustment 1.3 -y
```

This command will destroy the option-lockers, recover rewards and unlock deposits/withdrawals.

### 9. Query Staker Info

```bash
gaiad query wasm contract-state smart $LSM_CONTRACT \
  '{
    "staker_info": {
      "address": "cosmos1..."
    }
  }'
```

### 10. Query Proposal Status

```bash
gaiad query wasm contract-state smart $LSM_CONTRACT \
  '{"proposal_status":{}}'
```

## LSM (Liquid Staking Module) Overview

The Liquid Staking Module on Cosmos Hub allows users to tokenize their staked assets into LSM shares. These shares can be:

- Transferred to other addresses
- Used in DeFi protocols
- Redeemed back to staked tokens

### LSM Share Format

LSM shares have a special denom format:

```
{validator_address}/{tokenize_share_record_id}
```

Example: `cosmosvaloper1abc.../123`

### How This Contract Uses LSM

1. User deposits LSM shares to the contract (from ANY validator)
2. Contract validates the LSM denom format and verifies validator exists
3. Contract redeems the LSM shares (converts back to staked position)
4. The staking position is now controlled by the contract
5. User's stake is tracked internally (no receipt tokens)
6. User can claim rewards and withdraw at any time

### LSM Denom Validation

The contract performs strict validation on LSM denoms:

- Must be in format: `{validator_address}/{record_id}`
- Validator address must start with a valid prefix (cosmosvaloper, osmosisvaloper, etc.)
- Record ID must be a numeric value
- Validator must exist on-chain (queried via staking module)

This ensures only legitimate LSM shares are accepted.

## Testing

Run the test suite:

```bash
cargo test
```

The tests cover:

- Contract initialization
- LSM denom parsing and validation
- Valid and invalid LSM share formats
- Validator address validation
- Record ID validation
- Reward distribution algorithm
- Reward claiming
- Withdrawals
- State updates

## Security Considerations

1. **LSM Denom Validation**: The contract validates the LSM share format:
   - Correct format (validator/record_id)
   - Valid validator address prefix
   - Numeric record ID
   - Validator exists on-chain
2. **Single Token Deposits**: Only accepts one token per deposit to avoid confusion
3. **Overflow Protection**: Uses checked math operations
4. **Authorization**: Only the owner can update configuration and manage proposals
5. **Decimal Precision**: Uses `Decimal256` for high-precision reward calculations
6. **Zero Amount Checks**: Prevents operations with zero amounts
7. **Proposal Lock**: Blocks deposits/withdrawals while a proposal is active to guarantee vote integrity
8. **Option-Locker Isolation**: Each voting option is isolated in its own contract to prevent interference
9. **Automatic Voting**: Votes are automatic upon instantiation to avoid human errors
10. **Controlled Destruction**: Option-lockers can only be destroyed by the main LSM contract

## License

Apache-2.0

## Contributing

Contributions are welcome! Please ensure:

- All tests pass
- Code follows Rust formatting standards (`cargo fmt`)
- No clippy warnings (`cargo clippy`)


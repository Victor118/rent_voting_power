# LSM Staking Contract

A CosmWasm smart contract for managing Liquid Staking Module (LSM) shares from Cosmos Hub (Gaia). This contract allows users to deposit LSM shares, which are then redeemed and staked. It implements a cumulative reward index algorithm for fair reward distribution.

## Features

- **LSM Share Deposits**: Accept ANY valid LSM shares and automatically redeem them for staking
- **Dynamic Validator Support**: No need to configure specific validators - accepts LSM shares from any validator
- **Validator Verification**: Validates that the LSM share validator exists on-chain before accepting
- **Cumulative Reward Index Algorithm**: Fair and gas-efficient reward distribution
- **Reward Claiming**: Users can claim their accumulated staking rewards
- **Additional Reward Deposits**: Anyone can deposit additional rewards to be distributed
- **Staking Withdrawal**: Users can withdraw (unstake) their staked tokens
- **Admin Functions**: Contract owner can update configuration

## Architecture

The project is organized as a Cargo workspace:

```
lsm-staking/
├── contracts/
│   └── lsm-staking/         # Main contract
│       ├── src/
│       │   ├── contract.rs  # Contract logic (instantiate, execute, query)
│       │   ├── error.rs     # Error types
│       │   ├── state.rs     # State storage definitions
│       │   └── lib.rs       # Library exports
│       └── Cargo.toml
└── packages/
    └── lsm-types/           # Shared types and messages
        ├── src/
        │   └── lib.rs       # Message and state types
        └── Cargo.toml
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

### InstantiateMsg

```rust
{
  "staking_denom": "uatom",    // Base staking denom
  "owner": "cosmos1..."         // Contract admin
}
```

Note: Unlike many staking contracts, this contract does NOT require you to specify a fixed LSM denom. It accepts any valid LSM share token dynamically.

### ExecuteMsg

#### DepositLsmShares
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
3. Verify the validator address is valid (starts with cosmosvaloper or osmosisvaloper)
4. Verify the record_id is a valid number
5. Query the chain to ensure the validator exists
6. Redeem the LSM shares and stake them

#### ClaimRewards
Claim accumulated staking rewards:
```rust
{
  "claim_rewards": {}
}
```

#### DepositRewards
Deposit additional rewards to distribute to stakers:
```rust
{
  "deposit_rewards": {}
}
// Send staking tokens as funds
```

#### Withdraw
Withdraw (unstake) tokens from a validator:
```rust
{
  "withdraw": {
    "amount": "1000000",
    "validator": "cosmosvaloper1..."
  }
}
```

#### UpdateConfig
Update contract configuration (owner only):
```rust
{
  "update_config": {
    "owner": "cosmos1..."  // Optional
  }
}
```

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

### 1. Instantiate Contract

```bash
# Store the contract
RES=$(osmosisd tx wasm store artifacts/lsm_staking.wasm \
  --from wallet --gas auto --gas-adjustment 1.3 -y)

# Get code ID
CODE_ID=$(echo $RES | jq -r '.logs[0].events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')

# Instantiate
INIT_MSG='{
  "staking_denom": "uatom",
  "owner": "cosmos1..."
}'

osmosisd tx wasm instantiate $CODE_ID "$INIT_MSG" \
  --from wallet --label "LSM Staking" --gas auto --gas-adjustment 1.3 -y
```

### 2. Deposit LSM Shares

You can deposit LSM shares from ANY validator. The contract will validate the LSM denom format and verify the validator exists.

```bash
CONTRACT="cosmos1..."
# LSM shares can be from any validator - format: {validator}/{record_id}
LSM_DENOM="cosmosvaloper1abc123def456/789"

osmosisd tx wasm execute $CONTRACT \
  '{"deposit_lsm_shares":{}}' \
  --amount 1000000${LSM_DENOM} \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 3. Deposit Rewards

```bash
osmosisd tx wasm execute $CONTRACT \
  '{"deposit_rewards":{}}' \
  --amount 100000uatom \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 4. Claim Rewards

```bash
osmosisd tx wasm execute $CONTRACT \
  '{"claim_rewards":{}}' \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 5. Withdraw (Unstake)

```bash
osmosisd tx wasm execute $CONTRACT \
  '{
    "withdraw": {
      "amount": "500000",
      "validator": "cosmosvaloper1..."
    }
  }' \
  --from wallet --gas auto --gas-adjustment 1.3 -y
```

### 6. Query Staker Info

```bash
osmosisd query wasm contract-state smart $CONTRACT \
  '{
    "staker_info": {
      "address": "cosmos1..."
    }
  }'
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

1. **Dynamic LSM Denom Validation**: The contract validates ANY LSM share denom format and verifies:
   - Correct format (validator/record_id)
   - Valid validator address prefix
   - Numeric record ID
   - Validator exists on-chain
2. **Single Token Deposits**: Only accepts exactly one token per deposit to prevent confusion
3. **Overflow Protection**: Uses checked math operations throughout
4. **Authorization**: Only owner can update contract configuration
5. **Decimal Precision**: Uses `Decimal256` for high-precision reward calculations
6. **Zero Amount Checks**: Prevents operations with zero amounts

## License

Apache-2.0

## Contributing

Contributions are welcome! Please ensure:
- All tests pass
- Code follows Rust formatting standards (`cargo fmt`)
- No clippy warnings (`cargo clippy`)

## Future Enhancements

Potential improvements for future versions:
- Multi-validator support
- Auto-compounding rewards
- Governance integration
- Emergency pause mechanism
- Validator performance tracking
- Slashing protection mechanisms

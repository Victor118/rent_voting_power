# Deployment Guide

## Prerequisites

1. **Install Gaia** (Cosmos Hub binary)
   ```bash
   # Follow instructions at: https://hub.cosmos.network/main/getting-started/installation.html
   ```

2. **Build optimized WASM files**
   ```bash
   make optimize
   ```

3. **Install jq** (JSON processor)
   ```bash
   # Ubuntu/Debian
   sudo apt install jq

   # macOS
   brew install jq
   ```

## DevNet Deployment

### Quick Start

1. **Build the contracts**
   ```bash
   make optimize
   ```

2. **Run the deployment script**
   ```bash
   ./deploy-devnet.sh
   ```

The script will:
- ✓ Check if `gaiad` is installed
- ✓ Check/create the wallet `devnet-deployer`
- ✓ Verify wallet balance
- ✓ Upload both contract WASMs
- ✓ Instantiate LSM Staking contract with Proposal Locker code_id
- ✓ Save deployment info to `deployment-info.json`

### Manual Wallet Setup (if needed)

If you need to create and fund the wallet manually:

```bash
# Create wallet
gaiad keys add devnet-deployer --keyring-backend test

# Get wallet address
gaiad keys show devnet-deployer --keyring-backend test -a

# Fund wallet (from faucet or another account)
# For local devnet, you can use the genesis account
gaiad tx bank send <genesis-account> <devnet-deployer-address> 10000000uatom \
  --keyring-backend test \
  --chain-id localnet-1 \
  --node http://localhost:26657
```

### Configuration

The script uses these default values:
- **Chain ID**: `localnet-1`
- **Node**: `http://localhost:26657`
- **Keyring Backend**: `test`
- **Wallet**: `devnet-deployer`
- **Gas Prices**: `0.025uatom`

To customize, edit the variables at the top of `deploy-devnet.sh`.

### Deployment Output

After successful deployment, you'll get:

1. **Console output** with all contract addresses and code IDs
2. **deployment-info.json** with complete deployment information:
   ```json
   {
     "network": "devnet",
     "chain_id": "localnet-1",
     "contracts": {
       "proposal_locker": {
         "code_id": 1
       },
       "lsm_staking": {
         "code_id": 2,
         "address": "cosmos1..."
       }
     }
   }
   ```

## Post-Deployment

### Query Contract Info

```bash
# Get contract info
gaiad query wasm contract <contract-address> \
  --node http://localhost:26657

# Query contract state
gaiad query wasm contract-state smart <contract-address> \
  '{"config":{}}' \
  --node http://localhost:26657
```

### Interact with Contracts

#### Deposit LSM Shares
```bash
gaiad tx wasm execute <lsm-staking-address> \
  '{"deposit_lsm_shares":{}}' \
  --from devnet-deployer \
  --keyring-backend test \
  --amount 1000000cosmosvaloper1.../123 \
  --node http://localhost:26657 \
  --chain-id localnet-1
```

#### Deposit Rewards
```bash
gaiad tx wasm execute <lsm-staking-address> \
  '{"deposit_rewards":{}}' \
  --from devnet-deployer \
  --keyring-backend test \
  --amount 100000uatom \
  --node http://localhost:26657 \
  --chain-id localnet-1
```

#### Claim Rewards
```bash
gaiad tx wasm execute <lsm-staking-address> \
  '{"claim_rewards":{}}' \
  --from devnet-deployer \
  --keyring-backend test \
  --node http://localhost:26657 \
  --chain-id localnet-1
```

#### Withdraw
```bash
gaiad tx wasm execute <lsm-staking-address> \
  '{"withdraw":{"amount":"500000","validator":"cosmosvaloper1..."}}' \
  --from devnet-deployer \
  --keyring-backend test \
  --node http://localhost:26657 \
  --chain-id localnet-1
```

## Troubleshooting

### Wallet not found
```bash
# Create the wallet
gaiad keys add devnet-deployer --keyring-backend test
```

### Insufficient balance
```bash
# Check balance
gaiad query bank balances $(gaiad keys show devnet-deployer -a --keyring-backend test) \
  --node http://localhost:26657

# Fund from another account or faucet
```

### Node not responding
```bash
# Check if node is running
curl http://localhost:26657/status

# Start your local devnet if needed
gaiad start
```

### Transaction timeout
- Increase gas adjustment in the script
- Check network congestion
- Verify node is synced

## Production Deployment

For testnet/mainnet deployment:

1. **Update configuration** in `deploy-devnet.sh`:
   ```bash
   CHAIN_ID="cosmoshub-4"  # or theta-testnet-001
   NODE="https://rpc.cosmos.network:443"
   KEYRING_BACKEND="os"  # Use OS keyring for production
   WALLET_NAME="production-deployer"
   ```

2. **Use hardware wallet** for production deployments

3. **Audit contracts** before mainnet deployment

4. **Test on testnet** first

## Scripts Overview

- `deploy-devnet.sh` - Main deployment script
- `Makefile` - Build and optimize contracts
- `deployment-info.json` - Generated deployment information

## Support

For issues or questions:
- Check contract logs: `gaiad query wasm contract-history <address>`
- Review transaction: `gaiad query tx <tx-hash>`
- Examine events: `gaiad query tx <tx-hash> --output json | jq .events`

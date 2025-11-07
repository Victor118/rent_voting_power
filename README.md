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

Ce projet implémente un système de location de voting power pour la gouvernance du Cosmos Hub. Il se compose de deux types de contrats intelligents:

### 1. LSM-Staking Contract (Contrat Principal)
Le contrat principal qui gère les dépôts d'atoms et la location du voting power.

### 2. Proposal-Option-Locker Contracts
Des contrats secondaires instantiés dynamiquement pour chaque option de vote d'une proposition de gouvernance.

## Comment ça Fonctionne

### Phase 1: Dépôt et Staking Normal

1. **Dépôt de LSM Shares**: Les utilisateurs déposent des LSM shares du validateur spécifié lors de l'instantiation
2. **Redeem Automatique**: Dès réception, les LSM shares sont automatiquement redeem
3. **Custody des Atoms**: Le contrat conserve la custody des atoms en staking à tout moment
4. **Gestion des Rewards**: Les utilisateurs peuvent claim leurs rewards ou withdraw leurs LSM shares à tout moment

### Phase 2: Ouverture d'une Proposition de Gouvernance

1. **Ouverture par l'Admin**: Un admin peut ouvrir la location du voting power pour une proposition spécifique
2. **Instantiation des Option-Lockers**: Le système instantie autant de contrats "proposal-option-locker" qu'il y a d'options possibles:
   - YES locker → vote YES
   - NO locker → vote NO
   - NO_WITH_VETO locker → vote NO_WITH_VETO
   - ABSTAIN locker → vote ABSTAIN
3. **Vote Automatique**: Chaque contrat option-locker vote automatiquement pour son option respective lors de son instantiation
4. **Blocage des Opérations**: Pendant qu'une proposition est en cours, les utilisateurs ne peuvent plus deposer ou retirer

### Phase 3: Location du Voting Power

1. **Location par les Utilisateurs**: Un utilisateur peut louer du voting power d'atoms en choisissant une option à soutenir
2. **Tokenization**: Le montant d'atoms choisi est tokenizé
3. **Transfert au Option-Locker**: Les tokens sont transférés au smart contract de l'option choisie
4. **Redeem et Vote**: Le contrat option-locker redeem ces shares, ce qui augmente le vote alloué à cette option

### Phase 4: Fin de la Proposition

1. **Destruction des Option-Lockers**: Quand la proposition est terminée, les contrats option-locker peuvent être détruits
2. **Récupération des Rewards**: Chaque option-locker récupère les rewards de staking accumulés
3. **Envoi au LSM Contract**: Les rewards sont envoyés au contrat LSM principal
4. **Tokenization et Redeem**: Les option-lockers tokenizent tous leurs atoms stakés et les envoient au contrat LSM qui les redeem
5. **Déblocage**: Le contrat LSM redébloque les opérations de deposit et withdraw

## Features

- **LSM Share Deposits**: Accepte les LSM shares du validateur spécifié à l'instantiation, les redeem automatiquement
- **Validator Verification**: Valide que le validateur LSM share existe on-chain avant d'accepter
- **Cumulative Reward Index Algorithm**: Distribution équitable et gas-efficient des rewards
- **Reward Claiming**: Les utilisateurs peuvent claim leurs rewards de staking accumulés
- **Staking Withdrawal**: Les utilisateurs peuvent withdraw (unstake) leurs tokens, ils reçoivent des LSM shares
- **Voting Power Rental**: Location du voting power pour les propositions de gouvernance
- **Dynamic Proposal Option Lockers**: Instantiation dynamique de contrats pour chaque option de vote
- **Automatic Voting**: Vote automatique pour chaque option lors de l'instantiation des lockers
- **Operation Lock During Proposals**: Blocage des dépôts/retraits pendant les propositions actives
- **Admin Functions**: Le propriétaire du contrat peut mettre à jour la configuration et gérer les propositions

## Architecture

Le projet est organisé en workspace Cargo avec deux contrats:

```
rent_voting_power/
├── contracts/
│   ├── lsm-staking/              # Contrat principal
│   │   ├── src/
│   │   │   ├── contract.rs       # Logique du contrat (instantiate, execute, query)
│   │   │   ├── error.rs          # Types d'erreurs
│   │   │   ├── state.rs          # Définitions du stockage d'état
│   │   │   └── lib.rs            # Exports de la librairie
│   │   └── Cargo.toml
│   └── proposal-option-locker/   # Contrat option de vote
│       ├── src/
│       │   ├── contract.rs       # Logique du contrat option-locker
│       │   ├── error.rs          # Types d'erreurs
│       │   ├── state.rs          # Définitions du stockage d'état
│       │   └── lib.rs            # Exports de la librairie
│       └── Cargo.toml
└── packages/
    └── lsm-types/                # Types et messages partagés
        ├── src/
        │   └── lib.rs            # Types de messages et d'état
        └── Cargo.toml
```

### Flux de Données

```
Utilisateur
    ↓ (dépose LSM shares)
LSM-Staking Contract
    ↓ (redeem → atoms stakés)
Custody des atoms
    ↓ (proposition ouverte)
Instantiation de 4 Proposal-Option-Locker Contracts
    ├─ YES Option Locker (vote YES)
    ├─ NO Option Locker (vote NO)
    ├─ NO_WITH_VETO Option Locker (vote NO_WITH_VETO)
    └─ ABSTAIN Option Locker (vote ABSTAIN)
    ↓ (utilisateur loue voting power)
Tokenization des atoms → Transfert au Option-Locker choisi
    ↓ (redeem → augmente le vote)
Vote alloué à l'option
    ↓ (fin de proposition)
Destruction des Option-Lockers
    ↓ (récupération rewards + tokenization)
LSM-Staking Contract (redeem et déblocage)
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
  "validator": "cosmosvaloper1...", // Validateur pour les LSM shares
  "owner": "cosmos1..."             // Contract admin
}
```

#### ExecuteMsg

##### DepositLsmShares

Déposer des LSM shares pour être redeem et stakés:

```rust
{
  "deposit_lsm_shares": {}
}
// Envoyer EXACTEMENT UN token LSM share comme funds
// Format du denom LSM: {validator_address}/{record_id}
// Example: cosmosvaloper1abc.../123
```

Le contrat va:

1. Vérifier qu'exactement un token est envoyé
2. Parser et valider le format du denom LSM (validator/record_id)
3. Vérifier que l'adresse du validateur est valide
4. Vérifier que le record_id est un nombre valide
5. Redeem les LSM shares et les staker
6. **Bloqué si une proposition est en cours**

##### ClaimRewards

Réclamer les rewards de staking accumulés:

```rust
{
  "claim_rewards": {}
}
```

##### Withdraw

Retirer (unstake) des tokens d'un validateur:

```rust
{
  "withdraw": {
    "amount": "1000000",
    "validator": "cosmosvaloper1..."
  }
}
```

**Bloqué si une proposition est en cours**

##### OpenProposal

Ouvrir une proposition de gouvernance et instantier les option-lockers (admin only):

```rust
{
  "open_proposal": {
    "proposal_id": 123,
    "option_locker_code_id": 456  // Code ID du contrat proposal-option-locker
  }
}
```

Cette action va:
1. Instantier 4 contrats proposal-option-locker (YES, NO, NO_WITH_VETO, ABSTAIN)
2. Chaque contrat vote automatiquement pour son option
3. Bloquer les opérations de deposit et withdraw

##### RentVotingPower

Louer du voting power pour soutenir une option (utilisateur):

```rust
{
  "rent_voting_power": {
    "amount": "1000000",
    "option": "Yes"  // "Yes" | "No" | "NoWithVeto" | "Abstain"
  }
}
```

Cette action va:
1. Tokenizer le montant d'atoms spécifié
2. Transférer les tokens au contrat option-locker correspondant
3. Le option-locker redeem ces shares, augmentant le vote pour cette option

##### CloseProposal

Fermer une proposition et détruire les option-lockers (admin only):

```rust
{
  "close_proposal": {}
}
```

Cette action va:
1. Détruire tous les contrats option-locker
2. Récupérer les rewards de staking de chaque option-locker
3. Récupérer tous les atoms stakés (tokenizés puis redeem)
4. Débloquer les opérations de deposit et withdraw

##### UpdateConfig

Mettre à jour la configuration du contrat (owner only):

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
  "lsm_staking_contract": "cosmos1...",  // Adresse du contrat LSM principal
  "proposal_id": 123,                     // ID de la proposition
  "vote_option": "Yes",                   // "Yes" | "No" | "NoWithVeto" | "Abstain"
  "validator": "cosmosvaloper1..."        // Validateur à utiliser
}
```

Le contrat vote automatiquement pour l'option spécifiée lors de l'instantiation.

#### ExecuteMsg

##### ReceiveTokenizedShares

Recevoir des shares tokenizés du contrat LSM principal:

```rust
{
  "receive_tokenized_shares": {}
}
// Les shares sont automatiquement redeem pour augmenter le vote
```

##### Destroy

Détruire le contrat et retourner tous les assets au contrat LSM (admin only):

```rust
{
  "destroy": {}
}
```

Cette action va:
1. Claim tous les rewards de staking
2. Tokenizer tous les atoms stakés
3. Envoyer rewards et tokens tokenizés au contrat LSM principal

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

Cette commande va instantier 4 contrats option-locker et bloquer les dépôts/retraits.

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

Cette commande va détruire les option-lockers, récupérer les rewards et débloquer les dépôts/retraits.

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

1. **LSM Denom Validation**: Le contrat valide le format des LSM shares:
   - Format correct (validator/record_id)
   - Préfixe d'adresse validateur valide
   - Record ID numérique
   - Validateur existe on-chain
2. **Single Token Deposits**: N'accepte qu'un seul token par dépôt pour éviter la confusion
3. **Overflow Protection**: Utilise des opérations mathématiques vérifiées (checked math)
4. **Authorization**: Seul le propriétaire peut mettre à jour la configuration et gérer les propositions
5. **Decimal Precision**: Utilise `Decimal256` pour les calculs de rewards haute précision
6. **Zero Amount Checks**: Empêche les opérations avec des montants nuls
7. **Proposal Lock**: Bloque les dépôts/retraits pendant qu'une proposition est active pour garantir l'intégrité du vote
8. **Option-Locker Isolation**: Chaque option de vote est isolée dans son propre contrat pour éviter les interférences
9. **Automatic Voting**: Les votes sont automatiques lors de l'instantiation pour éviter les erreurs humaines
10. **Controlled Destruction**: Les option-lockers ne peuvent être détruits que par le contrat LSM principal

## License

Apache-2.0

## Contributing

Contributions are welcome! Please ensure:

- All tests pass
- Code follows Rust formatting standards (`cargo fmt`)
- No clippy warnings (`cargo clippy`)


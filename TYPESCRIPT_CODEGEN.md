# Génération des types TypeScript pour le frontend

Ce guide explique comment générer les types TypeScript à partir des smart contracts CosmWasm.

## Prérequis

Les schémas JSON ont déjà été générés dans `contracts/*/schema/`

## Installation de ts-codegen

```bash
npm install -g @cosmwasm/ts-codegen
```

Ou localement dans un projet frontend :
```bash
npm install --save-dev @cosmwasm/ts-codegen
```

## Génération des types TypeScript

### Option 1 : Génération simple (tous les contrats)

Créez un fichier `codegen.js` à la racine du projet :

```javascript
const codegen = require('@cosmwasm/ts-codegen').default;

codegen({
  contracts: [
    {
      name: 'LsmStaking',
      dir: './contracts/lsm-staking/schema'
    },
    {
      name: 'ProposalOptionLocker',
      dir: './contracts/proposal-option-locker/schema'
    }
  ],
  outPath: './ts-codegen/',

  // Options
  options: {
    bundle: {
      bundleFile: 'index.ts',
      scope: 'contracts'
    },
    types: {
      enabled: true
    },
    client: {
      enabled: true
    },
    reactQuery: {
      enabled: true,
      optionalClient: true,
      version: 'v4'
    },
    recoil: {
      enabled: false
    },
    messageComposer: {
      enabled: true
    },
    messageBuilder: {
      enabled: true
    },
    useContractsHooks: {
      enabled: true
    }
  }
}).then(() => {
  console.log('✨ TypeScript types generated successfully!');
});
```

Puis exécutez :
```bash
node codegen.js
```

### Option 2 : Script npm

Ajoutez dans `package.json` :

```json
{
  "scripts": {
    "codegen": "node codegen.js"
  },
  "devDependencies": {
    "@cosmwasm/ts-codegen": "^0.35.0"
  }
}
```

Puis :
```bash
npm run codegen
```

## Structure des fichiers générés

```
ts-codegen/
├── LsmStaking.types.ts          # Types TypeScript
├── LsmStaking.client.ts         # Client CosmJS
├── LsmStaking.message-composer.ts
├── LsmStaking.react-query.ts    # Hooks React Query
├── ProposalOptionLocker.types.ts
├── ProposalOptionLocker.client.ts
├── ProposalOptionLocker.message-composer.ts
├── ProposalOptionLocker.react-query.ts
└── index.ts                      # Export tout
```

## Utilisation dans un projet frontend

### Installation des dépendances

```bash
npm install @cosmjs/cosmwasm-stargate @cosmjs/stargate @cosmjs/proto-signing
npm install @tanstack/react-query  # Si vous utilisez reactQuery
```

### Exemple d'utilisation

```typescript
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { LsmStakingClient } from './ts-codegen/LsmStaking.client';

// Connexion au contrat
const client = await SigningCosmWasmClient.connectWithSigner(
  'https://rpc.cosmos.network',
  signer
);

const lsmStaking = new LsmStakingClient(
  client,
  senderAddress,
  contractAddress
);

// Appel des fonctions
await lsmStaking.depositLsmShares({}, 'auto', undefined, [
  { denom: 'cosmosvaloper1.../123', amount: '1000000' }
]);

const config = await lsmStaking.config();
console.log(config);

// Avec React Query (hooks)
import { useLsmStakingConfigQuery } from './ts-codegen/LsmStaking.react-query';

function MyComponent() {
  const { data: config, isLoading } = useLsmStakingConfigQuery({
    client,
    args: {}
  });

  return <div>{config?.validator}</div>;
}
```

## Intégration recommandée

Pour un projet frontend séparé :

1. **Créez un projet frontend** :
   ```bash
   npx create-next-app@latest staking-frontend
   # ou
   npm create vite@latest staking-frontend -- --template react-ts
   ```

2. **Copiez les types générés** :
   - Option A : Copiez `ts-codegen/` dans votre projet frontend
   - Option B : Publiez un package npm privé avec les types
   - Option C : Utilisez un monorepo (turborepo, nx)

3. **Installez les dépendances Cosmos** :
   ```bash
   npm install @cosmos-kit/react @cosmos-kit/keplr
   npm install @cosmjs/cosmwasm-stargate
   npm install @tanstack/react-query
   ```

## Régénération après modification des contrats

Chaque fois que vous modifiez les messages (InstantiateMsg, ExecuteMsg, QueryMsg) :

1. Régénérez les schémas :
   ```bash
   cd contracts/lsm-staking
   cargo run --example schema
   cd ../proposal-option-locker
   cargo run --example schema
   ```

2. Régénérez les types TypeScript :
   ```bash
   node codegen.js
   ```

3. Mettez à jour votre frontend

## Ressources

- [ts-codegen documentation](https://github.com/CosmWasm/ts-codegen)
- [CosmJS documentation](https://cosmos.github.io/cosmjs/)
- [Cosmos Kit](https://github.com/cosmology-tech/cosmos-kit)

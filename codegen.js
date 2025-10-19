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

  // Options de génération
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
  console.log('📁 Output directory: ./ts-codegen/');
  console.log('');
  console.log('📦 Files generated:');
  console.log('  - LsmStaking.types.ts');
  console.log('  - LsmStaking.client.ts');
  console.log('  - LsmStaking.react-query.ts');
  console.log('  - ProposalOptionLocker.types.ts');
  console.log('  - ProposalOptionLocker.client.ts');
  console.log('  - ProposalOptionLocker.react-query.ts');
  console.log('  - index.ts');
  console.log('');
  console.log('💡 Next steps:');
  console.log('  1. Copy ts-codegen/ to your frontend project');
  console.log('  2. Install dependencies: npm install @cosmjs/cosmwasm-stargate @tanstack/react-query');
  console.log('  3. Use the generated clients and hooks in your app!');
}).catch(err => {
  console.error('❌ Error generating types:', err);
  process.exit(1);
});

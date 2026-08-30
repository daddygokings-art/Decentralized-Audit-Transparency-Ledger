import { runWebUIConsumerTests } from './web-ui-consumer.test';
import { runSDKConsumerTests } from './sdk-consumer.test';
import { runBridgeRelayerConsumerTests } from './bridge-relayer-consumer.test';

async function main() {
  console.log('================ RUNNING PACT CONSUMER CONTRACT TESTS ================');
  try {
    const r1 = await runWebUIConsumerTests();
    const r2 = await runSDKConsumerTests();
    const r3 = await runBridgeRelayerConsumerTests();

    console.log('\n--- Consumer Test Results Summary ---');
    console.log(`[PASS] ${r1.suite}: ${r1.totalInteractions} interactions verified`);
    console.log(`[PASS] ${r2.suite}: ${r2.totalInteractions} interactions verified`);
    console.log(`[PASS] ${r3.suite}: ${r3.totalInteractions} interactions verified`);
    console.log('======================================================================\n');
  } catch (err: any) {
    console.error('Consumer test failed:', err.message);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

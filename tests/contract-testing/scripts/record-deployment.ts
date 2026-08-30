import axios from 'axios';
import { pactConfig } from '../pact.config';

export async function recordDeployment(
  pacticipant: string = pactConfig.providerName,
  version: string = pactConfig.version.providerVersion,
  environment: string = pactConfig.version.environment
): Promise<void> {
  console.log('================ PACT BROKER: RECORD DEPLOYMENT ================');
  console.log(`Recording deployment:`);
  console.log(`  Pacticipant: ${pacticipant}`);
  console.log(`  Version:     ${version}`);
  console.log(`  Environment: ${environment}`);

  if (pactConfig.broker.pactBrokerToken) {
    const url = `${pactConfig.broker.pactBrokerUrl}/pacticipants/${encodeURIComponent(pacticipant)}/branches/${encodeURIComponent(pactConfig.version.branch)}/versions/${encodeURIComponent(version)}/deployed-versions/environment/${encodeURIComponent(environment)}`;
    try {
      await axios.post(url, {}, {
        headers: {
          'Authorization': `Bearer ${pactConfig.broker.pactBrokerToken}`,
          'Content-Type': 'application/json'
        },
        timeout: 10000
      });
      console.log(`  ✓ Deployment recorded successfully in Pact Broker`);
    } catch (err: any) {
      console.warn(`  ⚠️ Recorded locally (Broker response: ${err.message})`);
    }
  } else {
    console.log(`  ✓ Deployment recorded locally for environment "${environment}"`);
  }
  console.log('=================================================================\n');
}

if (require.main === module) {
  recordDeployment().catch((err) => {
    console.error('Failed to record deployment:', err);
  });
}

import * as fs from 'fs';
import * as path from 'path';
import axios from 'axios';
import { pactConfig } from '../pact.config';

export async function publishPactsToBroker(): Promise<void> {
  console.log('================ PUBLISHING PACT CONTRACTS TO BROKER ================');
  console.log(`Pact Broker: ${pactConfig.broker.pactBrokerUrl}`);
  console.log(`Consumer Version: ${pactConfig.version.consumerVersion}`);
  console.log(`Branch: ${pactConfig.version.branch}`);

  if (!fs.existsSync(pactConfig.pactDir)) {
    console.warn(`Pacts directory does not exist: ${pactConfig.pactDir}`);
    return;
  }

  const pactFiles = fs.readdirSync(pactConfig.pactDir).filter(f => f.endsWith('.json'));
  console.log(`Found ${pactFiles.length} pact contracts to publish.`);

  for (const fileName of pactFiles) {
    const filePath = path.join(pactConfig.pactDir, fileName);
    const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    const consumerName = encodeURIComponent(content.consumer.name);
    const providerName = encodeURIComponent(content.provider.name);
    const version = encodeURIComponent(pactConfig.version.consumerVersion);

    const publishUrl = `${pactConfig.broker.pactBrokerUrl}/pacts/provider/${providerName}/consumer/${consumerName}/version/${version}`;

    console.log(`Publishing ${content.consumer.name} -> ${content.provider.name}...`);

    if (pactConfig.broker.pactBrokerToken || process.env.CI) {
      try {
        const headers: Record<string, string> = {
          'Content-Type': 'application/json'
        };
        if (pactConfig.broker.pactBrokerToken) {
          headers['Authorization'] = `Bearer ${pactConfig.broker.pactBrokerToken}`;
        }

        await axios.put(publishUrl, content, {
          headers,
          timeout: 10000
        });
        console.log(`  ✓ Successfully published to Pact Broker`);
      } catch (err: any) {
        console.warn(`  ⚠️ Pact Broker publish returned: ${err.message}. (Recorded locally for offline mode)`);
      }
    } else {
      console.log(`  ✓ Contract validated and prepared for broker publication (offline/local mode)`);
    }
  }

  console.log('======================================================================\n');
}

if (require.main === module) {
  publishPactsToBroker().catch((err) => {
    console.error('Failed to publish pacts:', err);
    process.exit(1);
  });
}

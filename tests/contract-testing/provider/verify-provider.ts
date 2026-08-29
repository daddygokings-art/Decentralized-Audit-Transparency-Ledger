import * as http from 'http';
import * as fs from 'fs';
import * as path from 'path';
import axios from 'axios';
import { createProviderApp } from './provider-server';
import { defaultStateContext, setupProviderState } from './provider-states';
import { pactConfig } from '../pact.config';

export async function verifyProviderContracts(): Promise<boolean> {
  console.log('================ RUNNING PACT PROVIDER VERIFICATION ================');
  console.log(`Provider: ${pactConfig.providerName}`);
  console.log(`Pacts Directory: ${pactConfig.pactDir}`);

  // 1. Start in-process provider server on configured port
  const app = createProviderApp(defaultStateContext);
  const server = http.createServer(app);

  await new Promise<void>((resolve) => {
    server.listen(pactConfig.providerPort, pactConfig.providerHost, () => {
      console.log(`[Provider Harness] Server running at ${pactConfig.providerBaseUrl}`);
      resolve();
    });
  });

  let allPassed = true;

  try {
    const pactFiles = fs.readdirSync(pactConfig.pactDir).filter(f => f.endsWith('.json'));

    for (const pactFileName of pactFiles) {
      const pactFilePath = path.join(pactConfig.pactDir, pactFileName);
      const pactContent = JSON.parse(fs.readFileSync(pactFilePath, 'utf8'));

      console.log(`\nVerifying contract: ${pactFileName} (Consumer: ${pactContent.consumer.name})`);

      for (const interaction of pactContent.interactions) {
        // Set up provider state if present
        if (interaction.providerState) {
          setupProviderState(interaction.providerState, defaultStateContext);
        }

        const requestUrl = `${pactConfig.providerBaseUrl}${interaction.request.path}${
          interaction.request.query ? `?${interaction.request.query}` : ''
        }`;

        try {
          const response = await axios({
            method: interaction.request.method,
            url: requestUrl,
            data: interaction.request.body,
            validateStatus: () => true // Allow all status codes
          });

          // Check status code
          if (response.status !== interaction.response.status) {
            console.error(`  ❌ Interaction failed: "${interaction.description}"`);
            console.error(`     Expected status ${interaction.response.status}, got ${response.status}`);
            allPassed = false;
          } else {
            console.log(`  ✓ Interaction passed: "${interaction.description}" (${response.status} OK)`);
          }
        } catch (err: any) {
          console.error(`  ❌ Error executing interaction: "${interaction.description}":`, err.message);
          allPassed = false;
        }
      }
    }
  } finally {
    server.close();
    console.log('[Provider Harness] Server closed.');
  }

  console.log('\n====================================================================');
  console.log(`Provider Verification Status: ${allPassed ? 'PASSED ✅' : 'FAILED ❌'}`);
  console.log('====================================================================\n');

  return allPassed;
}

if (require.main === module) {
  verifyProviderContracts().then((passed) => {
    if (!passed) {
      process.exit(1);
    }
  });
}

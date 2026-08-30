#!/usr/bin/env node
import { Command } from 'commander';
import { RunbookDefinition } from './types';
import { RunbookValidator } from './engine/validator';
import { RunbookRunner } from './engine/runner';
import { contractPauseRunbook } from './tasks/contract-pause';
import { capIncreaseRunbook } from './tasks/cap-increase';
import { schemaUpdateRunbook } from './tasks/schema-update';
import { bridgeFailoverRunbook } from './tasks/bridge-failover';

const program = new Command();

const runbooks: Record<string, RunbookDefinition> = {
  'contract-pause': contractPauseRunbook,
  'cap-increase': capIncreaseRunbook,
  'schema-update': schemaUpdateRunbook,
  'bridge-failover': bridgeFailoverRunbook,
};

program
  .name('runbook-cli')
  .description('Audit Ledger Operational Runbook Automation CLI')
  .version('1.0.0');

program
  .command('list')
  .description('List all available operational runbooks')
  .action(() => {
    console.log('\n=== Available Operational Runbooks ===\n');
    Object.entries(runbooks).forEach(([key, rb]) => {
      console.log(`• [${key}] ${rb.name} (v${rb.version}) - ${rb.description}`);
      console.log(`  Steps: ${rb.steps.length} | Type: ${rb.type} | Author: ${rb.author}\n`);
    });
  });

program
  .command('validate <runbookName>')
  .description('Validate preconditions and schema for a specific runbook')
  .option('-p, --params <json>', 'JSON string of input parameters', '{}')
  .action((runbookName, options) => {
    const rb = runbooks[runbookName];
    if (!rb) {
      console.error(`Error: Runbook "${runbookName}" not found.`);
      process.exit(1);
    }
    const params = JSON.parse(options.params);
    const report = RunbookValidator.validate(rb, params);
    console.log(JSON.stringify(report, null, 2));
    if (!report.isValid) {
      process.exit(1);
    }
  });

program
  .command('execute <runbookName>')
  .description('Execute an operational runbook')
  .option('--dry-run', 'Simulate execution without modifying ledger or cloud state', false)
  .option('-o, --operator <name>', 'Operator identifier', 'system-operator')
  .option('-c, --contract <address>', 'Target contract address', 'CCAU_SOROBAN_AUDIT_LEDGER')
  .option('-p, --params <json>', 'JSON string of input parameters', '{}')
  .action(async (runbookName, options) => {
    const rb = runbooks[runbookName];
    if (!rb) {
      console.error(`Error: Runbook "${runbookName}" not found.`);
      process.exit(1);
    }

    const params = JSON.parse(options.params);
    const summary = await RunbookRunner.execute(rb, {
      dryRun: options.dryRun,
      operator: options.operator,
      contractAddress: options.contract,
      params,
    });

    console.log('\n=== Execution Summary ===');
    console.log(JSON.stringify(summary, null, 2));
    if (summary.status === 'FAILED' || summary.status === 'ROLLED_BACK') {
      process.exit(1);
    }
  });

program.parse(process.argv);

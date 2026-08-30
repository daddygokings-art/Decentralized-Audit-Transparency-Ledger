#!/usr/bin/env node

import { MigrationRunner } from './engine';
import { SqliteAdapter } from './adapters/sqlite';
import { PostgresAdapter } from './adapters/postgres';
import { MysqlAdapter } from './adapters/mysql';
import { DatabaseAdapter } from './types';
import { ExpandContractEngine } from './zero-downtime/expandContract';
import { migration as m1 } from './migrations/001_contract_events_core';
import { migration as m2 } from './migrations/002_event_partitions_and_verification';
import { migration as m3 } from './migrations/003_event_aggregates_and_dead_letter';
import { migration as m4 } from './migrations/004_zero_downtime_payload_expansion';

function createAdapter(): DatabaseAdapter {
  const dialect = process.env.DB_DIALECT || 'postgres';
  if (dialect === 'postgres') {
    return new PostgresAdapter({
      connectionString: process.env.DATABASE_URL,
      host: process.env.DB_HOST,
      port: process.env.DB_PORT ? parseInt(process.env.DB_PORT, 10) : 5432,
      database: process.env.DB_NAME || 'audit_ledger',
      user: process.env.DB_USER || 'postgres',
      password: process.env.DB_PASSWORD,
    });
  } else if (dialect === 'mysql') {
    return new MysqlAdapter({
      host: process.env.DB_HOST || 'localhost',
      port: process.env.DB_PORT ? parseInt(process.env.DB_PORT, 10) : 3306,
      database: process.env.DB_NAME || 'audit_ledger',
      user: process.env.DB_USER || 'root',
      password: process.env.DB_PASSWORD,
    });
  }
  return new SqliteAdapter({ filename: process.env.DB_FILE || ':memory:' });
}

export async function main(args: string[] = process.argv.slice(2)): Promise<void> {
  const command = args[0] || 'status';
  const adapter = createAdapter();
  const runner = new MigrationRunner(adapter);

  // Register built-in migrations
  runner.registerMigrations([m1, m2, m3, m4]);

  try {
    switch (command) {
      case 'up': {
        const dryRun = args.includes('--dry-run');
        const stepsIdx = args.indexOf('--steps');
        const steps = stepsIdx >= 0 ? parseInt(args[stepsIdx + 1], 10) : undefined;
        const targetIdx = args.indexOf('--target-version');
        const targetVersion = targetIdx >= 0 ? args[targetIdx + 1] : undefined;

        console.log(`[audit-migrate] Running UP migrations (dryRun: ${dryRun})...`);
        const res = await runner.up({ steps, targetVersion, dryRun });
        if (res.dryRun) {
          console.log('[audit-migrate] Dry run complete. No changes applied.');
        } else {
          console.log(`[audit-migrate] Successfully applied ${res.applied.length} migration(s):`);
          for (const m of res.applied) {
            console.log(`  ✓ [Batch ${m.batch}] ${m.id} (${m.execution_time_ms}ms)`);
          }
        }
        break;
      }

      case 'down': {
        const dryRun = args.includes('--dry-run');
        const stepsIdx = args.indexOf('--steps');
        const steps = stepsIdx >= 0 ? parseInt(args[stepsIdx + 1], 10) : undefined;
        const targetIdx = args.indexOf('--target-version');
        const targetVersion = targetIdx >= 0 ? args[targetIdx + 1] : undefined;

        console.log(`[audit-migrate] Running DOWN rollback (dryRun: ${dryRun})...`);
        const res = await runner.down({ steps, targetVersion, dryRun });
        if (res.dryRun) {
          console.log('[audit-migrate] Dry run rollback complete.');
        } else {
          console.log(`[audit-migrate] Successfully rolled back ${res.rolledBack.length} migration(s):`);
          for (const m of res.rolledBack) {
            console.log(`  ↶ ${m.id}`);
          }
        }
        break;
      }

      case 'status': {
        const status = await runner.status();
        console.log('\n=== Contract Event Database Migration Status ===');
        console.log(`Total: ${status.migrations.length} | Applied: ${status.appliedCount} | Pending: ${status.pendingCount} | Latest Batch: ${status.latestBatch}`);
        console.log(`Checksum Integrity: ${status.checksumOk ? '✓ OK' : '✗ CHECKSUM MISMATCH DETECTED'}\n`);
        for (const m of status.migrations) {
          const icon = m.status === 'applied' ? '✓' : m.status === 'tampered' ? '✗' : '○';
          console.log(`  ${icon} ${m.id.padEnd(45)} [${m.status.toUpperCase()}] batch=${m.batch ?? '-'} phase=${m.phase}`);
        }
        console.log('');
        break;
      }

      case 'dry-run':
      case 'validate': {
        console.log('[audit-migrate] Validating migrations and checking locking hazards...');
        const validation = await runner.dryRunValidate('up');
        console.log(`Zero-Downtime Safe: ${validation.zeroDowntimeCompliant ? '✓ YES' : '✗ NO'}`);
        if (validation.warnings.length > 0) {
          console.log('\nWarnings & Lock Analyses:');
          for (const w of validation.warnings) {
            console.log(`  ! ${w}`);
          }
        }
        console.log(`\nPlanned migrations: ${validation.plannedMigrations.length}`);
        for (const p of validation.plannedMigrations) {
          console.log(`  • ${p.id} (${p.phase})`);
        }
        break;
      }

      case 'zero-downtime-plan': {
        const tableIdx = args.indexOf('--table');
        const oldColIdx = args.indexOf('--old-column');
        const newColIdx = args.indexOf('--new-column');
        const typeIdx = args.indexOf('--type');

        const table = tableIdx >= 0 ? args[tableIdx + 1] : 'contract_events';
        const oldCol = oldColIdx >= 0 ? args[oldColIdx + 1] : 'metadata';
        const newCol = newColIdx >= 0 ? args[newColIdx + 1] : 'metadata_v2';
        const colType = typeIdx >= 0 ? args[typeIdx + 1] : 'JSONB';

        const plan = ExpandContractEngine.planColumnMigration({
          table,
          oldColumn: oldCol,
          newColumn: newCol,
          columnType: colType,
          dialect: adapter.getDialect(),
        });

        console.log('\n=== Zero-Downtime Deployment Plan ===');
        console.log(`Target: ${table}.${oldCol} -> ${table}.${newCol} (${colType})`);
        console.log('\n--- Phase 1: Expand ---');
        console.log(plan.expand.statements.join('\n\n'));
        console.log('\n--- Phase 2: Backfill Script ---');
        console.log(ExpandContractEngine.generateBackfillScript(plan, adapter.getDialect()));
        console.log('\n--- Phase 3: Contract ---');
        console.log(plan.contract?.statements.join('\n\n'));
        console.log('\nPrerequisites:');
        plan.contract?.prerequisites.forEach((p) => console.log(`  - ${p}`));
        break;
      }

      default:
        console.log(`Unknown command: ${command}`);
        console.log('Usage: audit-migrate <up|down|status|dry-run|validate|zero-downtime-plan> [options]');
    }
  } finally {
    await adapter.close();
  }
}

if (require.main === module) {
  main().catch((err) => {
    console.error('[audit-migrate] Fatal error:', err);
    process.exit(1);
  });
}

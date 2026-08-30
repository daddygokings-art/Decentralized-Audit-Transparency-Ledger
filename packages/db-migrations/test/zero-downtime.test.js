const test = require('node:test');
const assert = require('node:assert');
const { LockAnalyzer } = require('../dist/zero-downtime/lockAnalyzer.js');
const { ExpandContractEngine } = require('../dist/zero-downtime/expandContract.js');
const { MigrationRunner } = require('../dist/engine.js');
const { SqliteAdapter } = require('../dist/adapters/sqlite.js');

test('Zero-Downtime Migration Analysis and Planning', async (t) => {
  await t.test('identifies high risk lock statements', () => {
    const analyses = LockAnalyzer.analyzeStatements(`
      ALTER TABLE contract_events ADD COLUMN new_col VARCHAR(64) NOT NULL;
      ALTER TABLE contract_events DROP COLUMN old_col;
      CREATE INDEX idx_unsafe ON contract_events(event_type);
      CREATE INDEX CONCURRENTLY idx_safe ON contract_events(event_type);
    `, 'postgres');

    assert.strictEqual(analyses.length, 4);
    assert.strictEqual(analyses[0].risk, 'high');
    assert.strictEqual(analyses[0].potentialDowntime, true);
    assert.match(analyses[0].recommendation, /Add as NULLABLE/);

    assert.strictEqual(analyses[1].risk, 'high');
    assert.strictEqual(analyses[1].potentialDowntime, true);
    assert.match(analyses[1].recommendation, /expand\/contract/);

    assert.strictEqual(analyses[2].risk, 'medium');
    assert.match(analyses[2].recommendation, /CONCURRENTLY/);

    assert.strictEqual(analyses[3].risk, 'low');
    assert.strictEqual(analyses[3].potentialDowntime, false);
  });

  await t.test('generates expand-contract zero-downtime plan and backfill script', () => {
    const plan = ExpandContractEngine.planColumnMigration({
      table: 'contract_events',
      oldColumn: 'metadata',
      newColumn: 'metadata_v2',
      columnType: 'JSONB',
      dialect: 'postgres',
    });

    assert.strictEqual(plan.phase, 'expand');
    assert.strictEqual(plan.expand.backwardCompatible, true);
    assert.ok(plan.expand.statements.some((s) => s.includes('ADD COLUMN IF NOT EXISTS metadata_v2')));
    assert.ok(plan.expand.statements.some((s) => s.includes('CREATE TRIGGER trg_sync_contract_events_metadata_v2')));

    assert.ok(plan.contract.statements.some((s) => s.includes('DROP COLUMN IF EXISTS metadata')));

    const backfillScript = ExpandContractEngine.generateBackfillScript(plan, 'postgres');
    assert.ok(backfillScript.includes('LIMIT 1000'));
    assert.ok(backfillScript.includes('SKIP LOCKED'));
    assert.ok(backfillScript.includes('pg_sleep'));
  });

  await t.test('dry-run validation validates plans without executing changes', async () => {
    const adapter = new SqliteAdapter();
    const runner = new MigrationRunner(adapter);

    runner.registerMigration({
      id: '001_test',
      version: '001',
      name: 'Safe migration',
      sqlUp: 'CREATE TABLE events_test (id INT PRIMARY KEY);',
      sqlDown: 'DROP TABLE events_test;',
      up: async () => {},
      down: async () => {},
    });

    const validation = await runner.dryRunValidate('up');
    assert.strictEqual(validation.valid, true);
    assert.strictEqual(validation.zeroDowntimeCompliant, true);
    assert.strictEqual(validation.plannedMigrations.length, 1);

    // Verify dry run did not execute anything
    const hasTable = await adapter.hasTable('events_test');
    assert.strictEqual(hasTable, false);
  });
});

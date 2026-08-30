const test = require('node:test');
const assert = require('node:assert');
const { SqliteAdapter } = require('../dist/adapters/sqlite.js');
const { PostgresAdapter } = require('../dist/adapters/postgres.js');
const { MigrationRunner } = require('../dist/engine.js');
const { BaseDatabaseAdapter } = require('../dist/adapters/base.js');

test('Database Migrations - Engine and Lifecycle', async (t) => {
  await t.test('executes versioned migrations up and tracks batches', async () => {
    const adapter = new SqliteAdapter();
    const runner = new MigrationRunner(adapter);

    runner.registerMigration({
      id: '001_initial',
      version: '001',
      name: 'Initial table',
      sqlUp: 'CREATE TABLE contract_events (id VARCHAR(64) PRIMARY KEY, event_hash VARCHAR(64));',
      sqlDown: 'DROP TABLE contract_events;',
      up: async () => {},
      down: async () => {},
    });

    runner.registerMigration({
      id: '002_add_index',
      version: '002',
      name: 'Add index',
      sqlUp: 'CREATE INDEX idx_hash ON contract_events(event_hash);',
      sqlDown: 'DROP INDEX idx_hash;',
      up: async () => {},
      down: async () => {},
    });

    const statusBefore = await runner.status();
    assert.strictEqual(statusBefore.pendingCount, 2);
    assert.strictEqual(statusBefore.appliedCount, 0);

    const upResult = await runner.up();
    assert.strictEqual(upResult.applied.length, 2);
    assert.strictEqual(upResult.applied[0].batch, 1);
    assert.strictEqual(upResult.applied[1].batch, 1);

    const statusAfter = await runner.status();
    assert.strictEqual(statusAfter.pendingCount, 0);
    assert.strictEqual(statusAfter.appliedCount, 2);
    assert.strictEqual(statusAfter.latestBatch, 1);
    assert.strictEqual(statusAfter.checksumOk, true);

    const hasTable = await adapter.hasTable('contract_events');
    assert.strictEqual(hasTable, true);
  });

  await t.test('supports stepwise rollback and batch rollback', async () => {
    const adapter = new SqliteAdapter();
    const runner = new MigrationRunner(adapter);

    runner.registerMigration({
      id: '001_first',
      version: '001',
      name: 'First',
      sqlUp: 'CREATE TABLE t1 (id INT PRIMARY KEY);',
      sqlDown: 'DROP TABLE t1;',
      up: async () => {},
      down: async () => {},
    });

    runner.registerMigration({
      id: '002_second',
      version: '002',
      name: 'Second',
      sqlUp: 'CREATE TABLE t2 (id INT PRIMARY KEY);',
      sqlDown: 'DROP TABLE t2;',
      up: async () => {},
      down: async () => {},
    });

    await runner.up({ steps: 1 });
    let status = await runner.status();
    assert.strictEqual(status.appliedCount, 1);
    assert.strictEqual(status.latestBatch, 1);

    // Apply second in batch 2
    await runner.up();
    status = await runner.status();
    assert.strictEqual(status.appliedCount, 2);
    assert.strictEqual(status.latestBatch, 2);

    // Rollback batch 2 (latest)
    const downResult = await runner.down();
    assert.strictEqual(downResult.rolledBack.length, 1);
    assert.strictEqual(downResult.rolledBack[0].id, '002_second');

    status = await runner.status();
    assert.strictEqual(status.appliedCount, 1);
    assert.strictEqual(status.pendingCount, 1);
  });

  await t.test('detects checksum tampering after migration execution', async () => {
    const adapter = new SqliteAdapter();
    const runner = new MigrationRunner(adapter);

    const mig = {
      id: '001_secure',
      version: '001',
      name: 'Secure migration',
      sqlUp: 'CREATE TABLE secure_events (id INT PRIMARY KEY);',
      sqlDown: 'DROP TABLE secure_events;',
      up: async () => {},
      down: async () => {},
    };

    runner.registerMigration(mig);
    await runner.up();

    // Create a new runner with a tampered version of 001_secure
    const runner2 = new MigrationRunner(adapter);
    runner2.registerMigration({
      id: '001_secure',
      version: '001',
      name: 'Secure migration',
      sqlUp: 'CREATE TABLE secure_events (id INT PRIMARY KEY, tampered INT);',
      sqlDown: 'DROP TABLE secure_events;',
      up: async () => {},
      down: async () => {},
    });

    const status = await runner2.status();
    assert.strictEqual(status.checksumOk, false);
    assert.strictEqual(status.migrations[0].status, 'tampered');

    await assert.rejects(
      async () => {
        await runner2.up();
      },
      /Checksum mismatch/
    );
  });

  await t.test('handles distributed locking prevents concurrent runs', async () => {
    const adapter = new PostgresAdapter();
    await adapter.connect();

    const lockAcquired = await adapter.acquireLock('migration_lock_test');
    assert.strictEqual(lockAcquired, true);

    // Second acquisition should fail
    const secondAcquire = await adapter.acquireLock('migration_lock_test');
    assert.strictEqual(secondAcquire, false);

    // Release lock
    const released = await adapter.releaseLock('migration_lock_test');
    assert.strictEqual(released, true);

    // Should be able to acquire again
    const thirdAcquire = await adapter.acquireLock('migration_lock_test');
    assert.strictEqual(thirdAcquire, true);
    await adapter.releaseLock('migration_lock_test');
  });

  await t.test('rolls back transaction atomically on failure during up', async () => {
    const adapter = new SqliteAdapter();
    const runner = new MigrationRunner(adapter);

    runner.registerMigration({
      id: '001_failing',
      version: '001',
      name: 'Failing step',
      sqlUp: 'CREATE TABLE fail_table (id INT PRIMARY KEY);',
      sqlDown: 'DROP TABLE fail_table;',
      up: async () => {
        throw new Error('Simulation of unexpected failure during up() execution');
      },
      down: async () => {},
    });

    await assert.rejects(
      async () => {
        await runner.up();
      },
      /Simulation of unexpected failure/
    );

    const status = await runner.status();
    assert.strictEqual(status.appliedCount, 0);
    assert.strictEqual(status.pendingCount, 1);
  });
});

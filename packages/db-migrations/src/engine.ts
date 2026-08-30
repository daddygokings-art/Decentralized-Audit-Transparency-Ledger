import {
  DatabaseAdapter,
  MigrationDefinition,
  MigrationRecord,
  MigrationRunnerOptions,
  DryRunResult,
  ZeroDowntimePlan,
  LockAnalysis,
} from './types';
import { BaseDatabaseAdapter } from './adapters/base';
import { LockAnalyzer } from './zero-downtime/lockAnalyzer';

export interface UpOptions {
  steps?: number;
  targetVersion?: string | number;
  dryRun?: boolean;
}

export interface DownOptions {
  steps?: number;
  targetVersion?: string | number;
  dryRun?: boolean;
  batchOnly?: boolean;
}

export interface MigrationStatusReport {
  appliedCount: number;
  pendingCount: number;
  latestBatch: number;
  checksumOk: boolean;
  migrations: Array<{
    id: string;
    version: string;
    name: string;
    status: 'applied' | 'pending' | 'tampered';
    batch?: number;
    appliedAt?: Date | string;
    executionTimeMs?: number;
    phase?: string;
  }>;
}

export class MigrationRunner {
  private migrations: MigrationDefinition[] = [];
  private options: Required<MigrationRunnerOptions>;

  constructor(
    private adapter: DatabaseAdapter,
    options: MigrationRunnerOptions = {}
  ) {
    this.options = {
      tableName: options.tableName || '_contract_event_migrations',
      lockTimeoutMs: options.lockTimeoutMs || 10000,
      migrationsDir: options.migrationsDir || './migrations',
      dryRun: options.dryRun || false,
      allowChecksumMismatch: options.allowChecksumMismatch || false,
    };
  }

  public registerMigration(migration: MigrationDefinition): void {
    if (!migration.checksum && migration.sqlUp) {
      migration.checksum = BaseDatabaseAdapter.calculateChecksum(migration.sqlUp);
    } else if (!migration.checksum) {
      migration.checksum = BaseDatabaseAdapter.calculateChecksum(
        `${migration.id}:${migration.name}:${migration.version}`
      );
    }

    const existing = this.migrations.findIndex((m) => m.id === migration.id);
    if (existing >= 0) {
      this.migrations[existing] = migration;
    } else {
      this.migrations.push(migration);
    }

    this.migrations.sort((a, b) => {
      const vA = String(a.version).padStart(10, '0');
      const vB = String(b.version).padStart(10, '0');
      return vA.localeCompare(vB);
    });
  }

  public registerMigrations(migrations: MigrationDefinition[]): void {
    for (const m of migrations) {
      this.registerMigration(m);
    }
  }

  public getRegisteredMigrations(): MigrationDefinition[] {
    return [...this.migrations];
  }

  /**
   * Runs pending migrations in sequential order.
   */
  public async up(opts: UpOptions = {}): Promise<{ applied: MigrationRecord[]; dryRun?: boolean }> {
    const dryRun = opts.dryRun ?? this.options.dryRun;
    await this.adapter.connect();

    const lockKey = `${this.options.tableName}_lock`;
    const lockAcquired = await this.adapter.acquireLock(lockKey, this.options.lockTimeoutMs);
    if (!lockAcquired) {
      throw new Error(`Failed to acquire distributed migration lock '${lockKey}'. Another migration process may be running.`);
    }

    try {
      await this.adapter.ensureMigrationTable(this.options.tableName);
      const appliedRecords = await this.adapter.getAppliedMigrations(this.options.tableName);
      const appliedMap = new Map(appliedRecords.map((r) => [r.id, r]));

      // Verify checksum integrity of previously applied migrations
      for (const m of this.migrations) {
        const applied = appliedMap.get(m.id);
        if (applied && applied.checksum && m.checksum && applied.checksum !== m.checksum) {
          const errMsg = `Checksum mismatch for applied migration '${m.id}'. Expected ${applied.checksum}, found ${m.checksum}. Migration file may have been modified after execution.`;
          if (!this.options.allowChecksumMismatch) {
            throw new Error(errMsg);
          }
        }
      }

      // Filter pending migrations
      let pending = this.migrations.filter((m) => !appliedMap.has(m.id));

      if (opts.targetVersion !== undefined) {
        const targetStr = String(opts.targetVersion);
        pending = pending.filter((m) => String(m.version) <= targetStr);
      }

      if (opts.steps !== undefined && opts.steps > 0) {
        pending = pending.slice(0, opts.steps);
      }

      if (dryRun) {
        return { applied: [], dryRun: true };
      }

      const nextBatch = (appliedRecords.reduce((max, r) => Math.max(max, r.batch), 0) || 0) + 1;
      const appliedInThisRun: MigrationRecord[] = [];

      for (const migration of pending) {
        const startTime = Date.now();
        const useTx = !migration.nonTransactional;

        if (useTx) {
          await this.adapter.beginTransaction();
        }

        try {
          if (migration.sqlUp) {
            await this.adapter.execute(migration.sqlUp);
          }
          await migration.up(this.adapter);

          const duration = Date.now() - startTime;
          const record: MigrationRecord = {
            id: migration.id,
            version: String(migration.version),
            name: migration.name,
            checksum: migration.checksum || '',
            batch: nextBatch,
            applied_at: new Date(),
            execution_time_ms: duration,
            status: 'applied',
          };

          await this.adapter.recordMigration(record, this.options.tableName);

          if (useTx) {
            await this.adapter.commitTransaction();
          }

          appliedInThisRun.push(record);
        } catch (err) {
          if (useTx) {
            try {
              await this.adapter.rollbackTransaction();
            } catch {
              // ignore rollback failure
            }
          }
          throw new Error(`Migration '${migration.id}' failed: ${(err as Error).message}`);
        }
      }

      return { applied: appliedInThisRun };
    } finally {
      await this.adapter.releaseLock(lockKey);
    }
  }

  /**
   * Rolls back applied migrations.
   */
  public async down(opts: DownOptions = {}): Promise<{ rolledBack: MigrationRecord[]; dryRun?: boolean }> {
    const dryRun = opts.dryRun ?? this.options.dryRun;
    await this.adapter.connect();

    const lockKey = `${this.options.tableName}_lock`;
    const lockAcquired = await this.adapter.acquireLock(lockKey, this.options.lockTimeoutMs);
    if (!lockAcquired) {
      throw new Error(`Failed to acquire distributed migration lock '${lockKey}'.`);
    }

    try {
      await this.adapter.ensureMigrationTable(this.options.tableName);
      const appliedRecords = await this.adapter.getAppliedMigrations(this.options.tableName);
      if (appliedRecords.length === 0) {
        return { rolledBack: [] };
      }

      let toRollback: MigrationRecord[] = [];
      const latestBatch = Math.max(...appliedRecords.map((r) => r.batch));

      if (opts.targetVersion !== undefined) {
        const targetStr = String(opts.targetVersion);
        toRollback = appliedRecords
          .filter((r) => String(r.version) > targetStr)
          .reverse();
      } else if (opts.steps !== undefined && opts.steps > 0) {
        toRollback = [...appliedRecords].reverse().slice(0, opts.steps);
      } else {
        // Default: roll back latest batch
        toRollback = appliedRecords
          .filter((r) => r.batch === latestBatch)
          .reverse();
      }

      if (dryRun) {
        return { rolledBack: toRollback, dryRun: true };
      }

      const rolledBackInThisRun: MigrationRecord[] = [];

      for (const record of toRollback) {
        const migration = this.migrations.find((m) => m.id === record.id);
        if (!migration) {
          throw new Error(`Cannot rollback migration '${record.id}' - migration definition not registered.`);
        }

        const useTx = !migration.nonTransactional;
        if (useTx) {
          await this.adapter.beginTransaction();
        }

        try {
          if (migration.sqlDown) {
            await this.adapter.execute(migration.sqlDown);
          }
          await migration.down(this.adapter);

          await this.adapter.removeMigration(record.id, this.options.tableName);

          if (useTx) {
            await this.adapter.commitTransaction();
          }

          rolledBackInThisRun.push(record);
        } catch (err) {
          if (useTx) {
            try {
              await this.adapter.rollbackTransaction();
            } catch {
              // ignore
            }
          }
          throw new Error(`Rollback of migration '${migration.id}' failed: ${(err as Error).message}`);
        }
      }

      return { rolledBack: rolledBackInThisRun };
    } finally {
      await this.adapter.releaseLock(lockKey);
    }
  }

  /**
   * Retrieves status report for all migrations.
   */
  public async status(): Promise<MigrationStatusReport> {
    await this.adapter.connect();
    await this.adapter.ensureMigrationTable(this.options.tableName);

    const appliedRecords = await this.adapter.getAppliedMigrations(this.options.tableName);
    const appliedMap = new Map(appliedRecords.map((r) => [r.id, r]));
    const latestBatch = appliedRecords.reduce((max, r) => Math.max(max, r.batch), 0) || 0;

    let checksumOk = true;
    const migrations = this.migrations.map((m) => {
      const applied = appliedMap.get(m.id);
      let status: 'applied' | 'pending' | 'tampered' = 'pending';

      if (applied) {
        if (applied.checksum && m.checksum && applied.checksum !== m.checksum) {
          status = 'tampered';
          checksumOk = false;
        } else {
          status = 'applied';
        }
      }

      return {
        id: m.id,
        version: String(m.version),
        name: m.name,
        status,
        batch: applied?.batch,
        appliedAt: applied?.applied_at,
        executionTimeMs: applied?.execution_time_ms,
        phase: m.phase || 'standard',
      };
    });

    return {
      appliedCount: appliedRecords.length,
      pendingCount: this.migrations.length - appliedRecords.length,
      latestBatch,
      checksumOk,
      migrations,
    };
  }

  /**
   * Performs dry-run validation of planned migrations without executing them.
   */
  public async dryRunValidate(direction: 'up' | 'down' = 'up'): Promise<DryRunResult> {
    await this.adapter.connect();
    await this.adapter.ensureMigrationTable(this.options.tableName);

    const appliedRecords = await this.adapter.getAppliedMigrations(this.options.tableName);
    const appliedMap = new Map(appliedRecords.map((r) => [r.id, r]));
    const dialect = this.adapter.getDialect();

    const plannedMigrations: DryRunResult['plannedMigrations'] = [];
    const lockAnalyses: LockAnalysis[] = [];
    const warnings: string[] = [];
    const errors: string[] = [];

    const targetList =
      direction === 'up'
        ? this.migrations.filter((m) => !appliedMap.has(m.id))
        : this.migrations
            .filter((m) => appliedMap.has(m.id))
            .reverse();

    for (const m of targetList) {
      const sql = (direction === 'up' ? m.sqlUp : m.sqlDown) || '';
      plannedMigrations.push({
        id: m.id,
        version: String(m.version),
        name: m.name,
        phase: m.phase || 'standard',
        sql,
      });

      if (sql) {
        const analyses = LockAnalyzer.analyzeStatements(sql, dialect);
        for (const analysis of analyses) {
          lockAnalyses.push(analysis);
          if (analysis.risk === 'high') {
            warnings.push(`[${m.id}] High lock risk: ${analysis.statement} -> ${analysis.recommendation}`);
          }
        }
      }
    }

    const hasHighRisk = lockAnalyses.some((a) => a.risk === 'high');

    return {
      valid: errors.length === 0,
      direction,
      plannedMigrations,
      lockAnalyses,
      warnings,
      errors,
      zeroDowntimeCompliant: !hasHighRisk,
    };
  }
}

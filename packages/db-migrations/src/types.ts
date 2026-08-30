/**
 * Database Migration Management Types
 * Supports PostgreSQL, SQLite, and MySQL with zero-downtime deployment capabilities.
 */

export type DatabaseDialect = 'postgres' | 'sqlite' | 'mysql';

export type MigrationStatus = 'pending' | 'applied' | 'rolled_back' | 'tampered' | 'failed';

export type MigrationPhase = 'expand' | 'backfill' | 'contract' | 'standard';

export interface DatabaseAdapter {
  getDialect(): DatabaseDialect;
  connect(): Promise<void>;
  close(): Promise<void>;
  query<T = any>(sql: string, params?: any[]): Promise<T[]>;
  execute(sql: string, params?: any[]): Promise<{ affectedRows?: number; lastInsertId?: any }>;
  beginTransaction(): Promise<void>;
  commitTransaction(): Promise<void>;
  rollbackTransaction(): Promise<void>;
  acquireLock(lockKey: string, timeoutMs?: number): Promise<boolean>;
  releaseLock(lockKey: string): Promise<boolean>;
  ensureMigrationTable(tableName?: string): Promise<void>;
  getAppliedMigrations(tableName?: string): Promise<MigrationRecord[]>;
  recordMigration(record: Omit<MigrationRecord, 'status'>, tableName?: string): Promise<void>;
  removeMigration(id: string, tableName?: string): Promise<void>;
  hasTable(tableName: string): Promise<boolean>;
  hasColumn(tableName: string, columnName: string): Promise<boolean>;
  getIndexes(tableName: string): Promise<string[]>;
}

export interface MigrationDefinition {
  id: string;
  version: number | string;
  name: string;
  up: (adapter: DatabaseAdapter) => Promise<void>;
  down: (adapter: DatabaseAdapter) => Promise<void>;
  sqlUp?: string;
  sqlDown?: string;
  checksum?: string;
  nonTransactional?: boolean;
  phase?: MigrationPhase;
  description?: string;
}

export interface MigrationRecord {
  id: string;
  version: string;
  name: string;
  checksum: string;
  batch: number;
  applied_at: Date | string;
  execution_time_ms: number;
  status: MigrationStatus;
}

export interface LockAnalysis {
  statement: string;
  lockLevel: 'EXCLUSIVE' | 'SHARE' | 'ROW_EXCLUSIVE' | 'NONE';
  potentialDowntime: boolean;
  risk: 'low' | 'medium' | 'high';
  recommendation?: string;
}

export interface DryRunResult {
  valid: boolean;
  direction: 'up' | 'down';
  plannedMigrations: Array<{
    id: string;
    version: string;
    name: string;
    phase: MigrationPhase;
    sql: string;
  }>;
  lockAnalyses: LockAnalysis[];
  warnings: string[];
  errors: string[];
  zeroDowntimeCompliant: boolean;
}

export interface ZeroDowntimePlan {
  migrationId: string;
  phase: MigrationPhase;
  expand: {
    statements: string[];
    description: string;
    backwardCompatible: boolean;
  };
  backfill?: {
    table: string;
    sourceColumn: string;
    targetColumn: string;
    batchSize: number;
    throttleMs: number;
    strategy: 'cursor' | 'offset' | 'dual_write';
  };
  contract?: {
    statements: string[];
    prerequisites: string[];
    gracePeriodHours?: number;
  };
}

export interface MigrationRunnerOptions {
  tableName?: string;
  lockTimeoutMs?: number;
  migrationsDir?: string;
  dryRun?: boolean;
  allowChecksumMismatch?: boolean;
}

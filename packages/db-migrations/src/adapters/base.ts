import { createHash } from 'crypto';
import { DatabaseAdapter, DatabaseDialect, MigrationRecord } from '../types';

export abstract class BaseDatabaseAdapter implements DatabaseAdapter {
  protected defaultTableName = '_contract_event_migrations';
  protected inTransaction = false;

  abstract getDialect(): DatabaseDialect;
  abstract connect(): Promise<void>;
  abstract close(): Promise<void>;
  abstract query<T = any>(sql: string, params?: any[]): Promise<T[]>;
  abstract execute(sql: string, params?: any[]): Promise<{ affectedRows?: number; lastInsertId?: any }>;
  abstract beginTransaction(): Promise<void>;
  abstract commitTransaction(): Promise<void>;
  abstract rollbackTransaction(): Promise<void>;
  abstract acquireLock(lockKey: string, timeoutMs?: number): Promise<boolean>;
  abstract releaseLock(lockKey: string): Promise<boolean>;
  abstract hasTable(tableName: string): Promise<boolean>;
  abstract hasColumn(tableName: string, columnName: string): Promise<boolean>;
  abstract getIndexes(tableName: string): Promise<string[]>;

  public static calculateChecksum(content: string): string {
    return createHash('sha256').update(content.trim().replace(/\r\n/g, '\n')).digest('hex');
  }

  public async ensureMigrationTable(tableName = this.defaultTableName): Promise<void> {
    const dialect = this.getDialect();
    let ddl: string;

    if (dialect === 'postgres') {
      ddl = `
        CREATE TABLE IF NOT EXISTS ${tableName} (
          id VARCHAR(255) PRIMARY KEY,
          version VARCHAR(64) NOT NULL,
          name VARCHAR(255) NOT NULL,
          checksum VARCHAR(64) NOT NULL,
          batch INTEGER NOT NULL,
          applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
          execution_time_ms INTEGER NOT NULL DEFAULT 0,
          status VARCHAR(32) NOT NULL DEFAULT 'applied'
        );
        CREATE INDEX IF NOT EXISTS idx_${tableName}_batch ON ${tableName}(batch);
        CREATE INDEX IF NOT EXISTS idx_${tableName}_version ON ${tableName}(version);
      `;
    } else if (dialect === 'mysql') {
      ddl = `
        CREATE TABLE IF NOT EXISTS ${tableName} (
          id VARCHAR(255) PRIMARY KEY,
          version VARCHAR(64) NOT NULL,
          name VARCHAR(255) NOT NULL,
          checksum VARCHAR(64) NOT NULL,
          batch INT NOT NULL,
          applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
          execution_time_ms INT NOT NULL DEFAULT 0,
          status VARCHAR(32) NOT NULL DEFAULT 'applied',
          INDEX idx_${tableName}_batch (batch),
          INDEX idx_${tableName}_version (version)
        );
      `;
    } else {
      // SQLite
      ddl = `
        CREATE TABLE IF NOT EXISTS ${tableName} (
          id TEXT PRIMARY KEY,
          version TEXT NOT NULL,
          name TEXT NOT NULL,
          checksum TEXT NOT NULL,
          batch INTEGER NOT NULL,
          applied_at TEXT NOT NULL DEFAULT (datetime('now')),
          execution_time_ms INTEGER NOT NULL DEFAULT 0,
          status TEXT NOT NULL DEFAULT 'applied'
        );
        CREATE INDEX IF NOT EXISTS idx_${tableName}_batch ON ${tableName}(batch);
        CREATE INDEX IF NOT EXISTS idx_${tableName}_version ON ${tableName}(version);
      `;
    }

    await this.execute(ddl);
  }

  public async getAppliedMigrations(tableName = this.defaultTableName): Promise<MigrationRecord[]> {
    await this.ensureMigrationTable(tableName);
    const sql = `SELECT id, version, name, checksum, batch, applied_at, execution_time_ms, status FROM ${tableName} ORDER BY batch ASC, id ASC`;
    const rows = await this.query<any>(sql);
    return rows.map((r) => ({
      id: String(r.id),
      version: String(r.version),
      name: String(r.name),
      checksum: String(r.checksum),
      batch: Number(r.batch),
      applied_at: r.applied_at,
      execution_time_ms: Number(r.execution_time_ms || 0),
      status: (r.status as any) || 'applied',
    }));
  }

  public async recordMigration(
    record: Omit<MigrationRecord, 'status'>,
    tableName = this.defaultTableName
  ): Promise<void> {
    const dialect = this.getDialect();
    let sql: string;
    const now = new Date().toISOString();

    if (dialect === 'postgres') {
      sql = `
        INSERT INTO ${tableName} (id, version, name, checksum, batch, applied_at, execution_time_ms, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'applied')
        ON CONFLICT (id) DO UPDATE SET
          checksum = EXCLUDED.checksum,
          batch = EXCLUDED.batch,
          applied_at = EXCLUDED.applied_at,
          execution_time_ms = EXCLUDED.execution_time_ms,
          status = 'applied'
      `;
    } else if (dialect === 'mysql') {
      sql = `
        INSERT INTO ${tableName} (id, version, name, checksum, batch, applied_at, execution_time_ms, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'applied')
        ON DUPLICATE KEY UPDATE
          checksum = VALUES(checksum),
          batch = VALUES(batch),
          applied_at = VALUES(applied_at),
          execution_time_ms = VALUES(execution_time_ms),
          status = 'applied'
      `;
    } else {
      sql = `
        INSERT OR REPLACE INTO ${tableName} (id, version, name, checksum, batch, applied_at, execution_time_ms, status)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'applied')
      `;
    }

    await this.execute(sql, [
      record.id,
      record.version,
      record.name,
      record.checksum,
      record.batch,
      now,
      record.execution_time_ms,
    ]);
  }

  public async removeMigration(id: string, tableName = this.defaultTableName): Promise<void> {
    const dialect = this.getDialect();
    const placeholder = dialect === 'postgres' ? '$1' : '?';
    const sql = `DELETE FROM ${tableName} WHERE id = ${placeholder}`;
    await this.execute(sql, [id]);
  }
}

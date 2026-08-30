import { BaseDatabaseAdapter } from './base';
import { DatabaseDialect } from '../types';

export interface SqliteAdapterOptions {
  filename?: string;
}

export class SqliteAdapter extends BaseDatabaseAdapter {
  private tables: Map<string, Array<Record<string, any>>> = new Map();
  private schema: Map<string, Set<string>> = new Map();
  private indexes: Map<string, Set<string>> = new Map();
  private locks: Set<string> = new Set();
  private transactionSavepoint: {
    tables: string;
    schema: string;
    indexes: string;
  } | null = null;
  private connected = false;

  constructor(private options: SqliteAdapterOptions = {}) {
    super();
  }

  public getDialect(): DatabaseDialect {
    return 'sqlite';
  }

  public async connect(): Promise<void> {
    this.connected = true;
  }

  public async close(): Promise<void> {
    this.connected = false;
  }

  public async query<T = any>(sql: string, params: any[] = []): Promise<T[]> {
    const trimmed = sql.trim().toLowerCase();
    
    // Check if selecting from migration table
    if (trimmed.includes('from _contract_event_migrations') || trimmed.includes(`from ${this.defaultTableName}`)) {
      const rows = this.tables.get(this.defaultTableName) || [];
      return rows.map((r) => ({ ...r })) as unknown as T[];
    }

    // Generic select matching table names
    for (const [tableName, rows] of this.tables.entries()) {
      if (trimmed.includes(`from ${tableName.toLowerCase()}`)) {
        return rows.map((r) => ({ ...r })) as unknown as T[];
      }
    }

    return [] as T[];
  }

  public async execute(sql: string, params: any[] = []): Promise<{ affectedRows?: number; lastInsertId?: any }> {
    const statements = sql
      .split(';')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);

    let affected = 0;

    for (const stmt of statements) {
      const lower = stmt.toLowerCase();

      // CREATE TABLE
      const createTableMatch = stmt.match(/create\s+table\s+(?:if\s+not\s+exists\s+)?([a-zA-Z0-9_]+)\s*\(([\s\S]+)\)/i);
      if (createTableMatch) {
        const tableName = createTableMatch[1];
        if (!this.tables.has(tableName)) {
          this.tables.set(tableName, []);
          this.schema.set(tableName, new Set());
          const cols = createTableMatch[2].split(',').map((c) => c.trim().split(/\s+/)[0]);
          for (const col of cols) {
            if (col && !col.toUpperCase().startsWith('PRIMARY') && !col.toUpperCase().startsWith('INDEX') && !col.toUpperCase().startsWith('FOREIGN')) {
              this.schema.get(tableName)?.add(col);
            }
          }
        }
        continue;
      }

      // CREATE INDEX
      const createIndexMatch = stmt.match(/create\s+(?:unique\s+)?index\s+(?:if\s+not\s+exists\s+)?([a-zA-Z0-9_]+)\s+on\s+([a-zA-Z0-9_]+)/i);
      if (createIndexMatch) {
        const indexName = createIndexMatch[1];
        const tableName = createIndexMatch[2];
        if (!this.indexes.has(tableName)) {
          this.indexes.set(tableName, new Set());
        }
        this.indexes.get(tableName)?.add(indexName);
        continue;
      }

      // ALTER TABLE ADD COLUMN
      const alterAddMatch = stmt.match(/alter\s+table\s+([a-zA-Z0-9_]+)\s+add\s+(?:column\s+)?([a-zA-Z0-9_]+)/i);
      if (alterAddMatch) {
        const tableName = alterAddMatch[1];
        const colName = alterAddMatch[2];
        if (!this.schema.has(tableName)) {
          this.schema.set(tableName, new Set());
        }
        this.schema.get(tableName)?.add(colName);
        continue;
      }

      // DROP TABLE
      const dropMatch = stmt.match(/drop\s+table\s+(?:if\s+exists\s+)?([a-zA-Z0-9_]+)/i);
      if (dropMatch) {
        const tableName = dropMatch[1];
        this.tables.delete(tableName);
        this.schema.delete(tableName);
        this.indexes.delete(tableName);
        continue;
      }

      // INSERT INTO migration table
      if (lower.startsWith('insert or replace into') || lower.startsWith('insert into')) {
        const insertMatch = stmt.match(/insert\s+(?:or\s+replace\s+)?into\s+([a-zA-Z0-9_]+)/i);
        if (insertMatch) {
          const tableName = insertMatch[1];
          if (!this.tables.has(tableName)) {
            this.tables.set(tableName, []);
          }
          const tableRows = this.tables.get(tableName)!;
          if (tableName === this.defaultTableName && params.length >= 7) {
            const [id, version, name, checksum, batch, applied_at, execution_time_ms] = params;
            const existingIdx = tableRows.findIndex((r) => r.id === id);
            const row = { id, version, name, checksum, batch, applied_at, execution_time_ms, status: 'applied' };
            if (existingIdx >= 0) {
              tableRows[existingIdx] = row;
            } else {
              tableRows.push(row);
            }
            affected++;
          }
        }
        continue;
      }

      // DELETE FROM migration table
      if (lower.startsWith('delete from')) {
        const deleteMatch = stmt.match(/delete\s+from\s+([a-zA-Z0-9_]+)/i);
        if (deleteMatch && params.length > 0) {
          const tableName = deleteMatch[1];
          const tableRows = this.tables.get(tableName);
          if (tableRows) {
            const id = params[0];
            const filtered = tableRows.filter((r) => r.id !== id);
            affected += tableRows.length - filtered.length;
            this.tables.set(tableName, filtered);
          }
        }
        continue;
      }
    }

    return { affectedRows: affected };
  }

  public async beginTransaction(): Promise<void> {
    if (this.inTransaction) throw new Error('Transaction already active');
    this.inTransaction = true;
    this.transactionSavepoint = {
      tables: JSON.stringify(Array.from(this.tables.entries())),
      schema: JSON.stringify(Array.from(this.schema.entries()).map(([k, v]) => [k, Array.from(v)])),
      indexes: JSON.stringify(Array.from(this.indexes.entries()).map(([k, v]) => [k, Array.from(v)])),
    };
  }

  public async commitTransaction(): Promise<void> {
    if (!this.inTransaction) throw new Error('No active transaction');
    this.inTransaction = false;
    this.transactionSavepoint = null;
  }

  public async rollbackTransaction(): Promise<void> {
    if (!this.inTransaction) throw new Error('No active transaction');
    if (this.transactionSavepoint) {
      this.tables = new Map(JSON.parse(this.transactionSavepoint.tables));
      const schemaData = JSON.parse(this.transactionSavepoint.schema) as [string, string[]][];
      this.schema = new Map(schemaData.map(([k, arr]) => [k, new Set(arr)]));
      const indexData = JSON.parse(this.transactionSavepoint.indexes) as [string, string[]][];
      this.indexes = new Map(indexData.map(([k, arr]) => [k, new Set(arr)]));
    }
    this.inTransaction = false;
    this.transactionSavepoint = null;
  }

  public async acquireLock(lockKey: string, _timeoutMs = 5000): Promise<boolean> {
    if (this.locks.has(lockKey)) {
      return false;
    }
    this.locks.add(lockKey);
    return true;
  }

  public async releaseLock(lockKey: string): Promise<boolean> {
    return this.locks.delete(lockKey);
  }

  public async hasTable(tableName: string): Promise<boolean> {
    return this.tables.has(tableName) || this.schema.has(tableName);
  }

  public async hasColumn(tableName: string, columnName: string): Promise<boolean> {
    const cols = this.schema.get(tableName);
    return cols ? cols.has(columnName) : false;
  }

  public async getIndexes(tableName: string): Promise<string[]> {
    const idxs = this.indexes.get(tableName);
    return idxs ? Array.from(idxs) : [];
  }
}

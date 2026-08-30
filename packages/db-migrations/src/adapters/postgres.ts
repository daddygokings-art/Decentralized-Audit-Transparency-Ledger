import { BaseDatabaseAdapter } from './base';
import { DatabaseDialect } from '../types';

export interface PostgresAdapterOptions {
  connectionString?: string;
  host?: string;
  port?: number;
  database?: string;
  user?: string;
  password?: string;
  client?: any; // Injected pg.Client / pg.Pool
}

export class PostgresAdapter extends BaseDatabaseAdapter {
  private client: any;
  private isExternalClient = false;
  private mockTables: Map<string, Array<Record<string, any>>> = new Map();
  private mockSchema: Map<string, Set<string>> = new Map();
  private mockIndexes: Map<string, Set<string>> = new Map();
  private mockLocks: Set<number> = new Set();
  private transactionDepth = 0;

  constructor(private options: PostgresAdapterOptions = {}) {
    super();
    if (options.client) {
      this.client = options.client;
      this.isExternalClient = true;
    }
  }

  public getDialect(): DatabaseDialect {
    return 'postgres';
  }

  public async connect(): Promise<void> {
    if (this.client && typeof this.client.connect === 'function') {
      await this.client.connect();
    }
  }

  public async close(): Promise<void> {
    if (!this.isExternalClient && this.client && typeof this.client.end === 'function') {
      await this.client.end();
    }
  }

  public async query<T = any>(sql: string, params: any[] = []): Promise<T[]> {
    if (this.client && typeof this.client.query === 'function') {
      const res = await this.client.query(sql, params);
      return res.rows || [];
    }

    // Mock query handler for standalone/test execution
    const trimmed = sql.trim().toLowerCase();
    if (trimmed.includes('from _contract_event_migrations') || trimmed.includes(`from ${this.defaultTableName}`)) {
      const rows = this.mockTables.get(this.defaultTableName) || [];
      return rows.map((r) => ({ ...r })) as unknown as T[];
    }

    for (const [tableName, rows] of this.mockTables.entries()) {
      if (trimmed.includes(`from ${tableName.toLowerCase()}`)) {
        return rows.map((r) => ({ ...r })) as unknown as T[];
      }
    }

    return [] as T[];
  }

  public async execute(sql: string, params: any[] = []): Promise<{ affectedRows?: number; lastInsertId?: any }> {
    if (this.client && typeof this.client.query === 'function') {
      const res = await this.client.query(sql, params);
      return { affectedRows: res.rowCount };
    }

    // Parse and track mock tables/columns
    const stmts = sql.split(';').map((s) => s.trim()).filter(Boolean);
    let affected = 0;

    for (const stmt of stmts) {
      const lower = stmt.toLowerCase();

      const createTableMatch = stmt.match(/create\s+table\s+(?:if\s+not\s+exists\s+)?([a-zA-Z0-9_]+)\s*\(([\s\S]+)\)/i);
      if (createTableMatch) {
        const tableName = createTableMatch[1];
        if (!this.mockTables.has(tableName)) {
          this.mockTables.set(tableName, []);
          this.mockSchema.set(tableName, new Set());
          const cols = createTableMatch[2].split(',').map((c) => c.trim().split(/\s+/)[0]);
          for (const col of cols) {
            if (col && !['PRIMARY', 'INDEX', 'FOREIGN', 'CONSTRAINT'].includes(col.toUpperCase())) {
              this.mockSchema.get(tableName)?.add(col);
            }
          }
        }
        continue;
      }

      const createIndexMatch = stmt.match(/create\s+(?:unique\s+)?index\s+(?:concurrently\s+)?(?:if\s+not\s+exists\s+)?([a-zA-Z0-9_]+)\s+on\s+([a-zA-Z0-9_]+)/i);
      if (createIndexMatch) {
        const indexName = createIndexMatch[1];
        const tableName = createIndexMatch[2];
        if (!this.mockIndexes.has(tableName)) {
          this.mockIndexes.set(tableName, new Set());
        }
        this.mockIndexes.get(tableName)?.add(indexName);
        continue;
      }

      const alterAddMatch = stmt.match(/alter\s+table\s+([a-zA-Z0-9_]+)\s+add\s+(?:column\s+)?([a-zA-Z0-9_]+)/i);
      if (alterAddMatch) {
        const tableName = alterAddMatch[1];
        const colName = alterAddMatch[2];
        if (!this.mockSchema.has(tableName)) {
          this.mockSchema.set(tableName, new Set());
        }
        this.mockSchema.get(tableName)?.add(colName);
        continue;
      }

      if (lower.startsWith('insert into') && lower.includes('_contract_event_migrations')) {
        if (!this.mockTables.has(this.defaultTableName)) {
          this.mockTables.set(this.defaultTableName, []);
        }
        const rows = this.mockTables.get(this.defaultTableName)!;
        if (params.length >= 7) {
          const [id, version, name, checksum, batch, applied_at, execution_time_ms] = params;
          const idx = rows.findIndex((r) => r.id === id);
          const record = { id, version, name, checksum, batch, applied_at, execution_time_ms, status: 'applied' };
          if (idx >= 0) rows[idx] = record;
          else rows.push(record);
          affected++;
        }
        continue;
      }

      if (lower.startsWith('delete from') && lower.includes('_contract_event_migrations')) {
        const rows = this.mockTables.get(this.defaultTableName);
        if (rows && params.length > 0) {
          const id = params[0];
          const filtered = rows.filter((r) => r.id !== id);
          affected += rows.length - filtered.length;
          this.mockTables.set(this.defaultTableName, filtered);
        }
        continue;
      }
    }

    return { affectedRows: affected };
  }

  public async beginTransaction(): Promise<void> {
    if (this.client) {
      await this.client.query('BEGIN');
    }
    this.inTransaction = true;
    this.transactionDepth++;
  }

  public async commitTransaction(): Promise<void> {
    if (this.client) {
      await this.client.query('COMMIT');
    }
    this.inTransaction = false;
    this.transactionDepth = 0;
  }

  public async rollbackTransaction(): Promise<void> {
    if (this.client) {
      await this.client.query('ROLLBACK');
    }
    this.inTransaction = false;
    this.transactionDepth = 0;
  }

  private hashKey(key: string): number {
    let hash = 0;
    for (let i = 0; i < key.length; i++) {
      hash = (hash << 5) - hash + key.charCodeAt(i);
      hash |= 0;
    }
    return hash;
  }

  public async acquireLock(lockKey: string, timeoutMs = 5000): Promise<boolean> {
    const lockId = this.hashKey(lockKey);
    if (this.client) {
      const res = await this.client.query('SELECT pg_try_advisory_lock($1) AS locked', [lockId]);
      return !!res.rows[0]?.locked;
    }

    if (this.mockLocks.has(lockId)) return false;
    this.mockLocks.add(lockId);
    return true;
  }

  public async releaseLock(lockKey: string): Promise<boolean> {
    const lockId = this.hashKey(lockKey);
    if (this.client) {
      const res = await this.client.query('SELECT pg_advisory_unlock($1) AS unlocked', [lockId]);
      return !!res.rows[0]?.unlocked;
    }
    return this.mockLocks.delete(lockId);
  }

  public async hasTable(tableName: string): Promise<boolean> {
    if (this.client) {
      const res = await this.client.query(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = $1) AS exists",
        [tableName]
      );
      return !!res.rows[0]?.exists;
    }
    return this.mockTables.has(tableName) || this.mockSchema.has(tableName);
  }

  public async hasColumn(tableName: string, columnName: string): Promise<boolean> {
    if (this.client) {
      const res = await this.client.query(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = $1 AND column_name = $2) AS exists",
        [tableName, columnName]
      );
      return !!res.rows[0]?.exists;
    }
    const cols = this.mockSchema.get(tableName);
    return cols ? cols.has(columnName) : false;
  }

  public async getIndexes(tableName: string): Promise<string[]> {
    if (this.client) {
      const res = await this.client.query(
        "SELECT indexname FROM pg_indexes WHERE tablename = $1",
        [tableName]
      );
      return res.rows.map((r: any) => r.indexname);
    }
    const idxs = this.mockIndexes.get(tableName);
    return idxs ? Array.from(idxs) : [];
  }
}

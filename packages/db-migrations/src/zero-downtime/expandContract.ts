import { ZeroDowntimePlan, DatabaseDialect } from '../types';

export interface ColumnRenamePlanOptions {
  table: string;
  oldColumn: string;
  newColumn: string;
  columnType: string;
  primaryKeyColumn?: string;
  batchSize?: number;
  throttleMs?: number;
  dialect?: DatabaseDialect;
}

export class ExpandContractEngine {
  /**
   * Generates a 3-phase zero-downtime deployment plan for renaming or transforming a column.
   * Phase 1: Expand (add new nullable column, add sync trigger or dual-write view)
   * Phase 2: Backfill (batched copy from old to new column without locking table)
   * Phase 3: Contract (drop old column and sync triggers after new app release is live)
   */
  public static planColumnMigration(options: ColumnRenamePlanOptions): ZeroDowntimePlan {
    const {
      table,
      oldColumn,
      newColumn,
      columnType,
      primaryKeyColumn = 'id',
      batchSize = 1000,
      throttleMs = 50,
      dialect = 'postgres',
    } = options;

    const expandStatements: string[] = [];
    const contractStatements: string[] = [];

    if (dialect === 'postgres') {
      // Phase 1: Expand
      expandStatements.push(`ALTER TABLE ${table} ADD COLUMN IF NOT EXISTS ${newColumn} ${columnType};`);
      expandStatements.push(`
CREATE OR REPLACE FUNCTION sync_${table}_${newColumn}()
RETURNS TRIGGER AS $$
BEGIN
  IF NEW.${newColumn} IS NULL AND NEW.${oldColumn} IS NOT NULL THEN
    NEW.${newColumn} = NEW.${oldColumn};
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;
      `.trim());
      expandStatements.push(`
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'trg_sync_${table}_${newColumn}') THEN
    CREATE TRIGGER trg_sync_${table}_${newColumn}
    BEFORE INSERT OR UPDATE ON ${table}
    FOR EACH ROW EXECUTE FUNCTION sync_${table}_${newColumn}();
  END IF;
END $$;
      `.trim());

      // Phase 3: Contract
      contractStatements.push(`DROP TRIGGER IF EXISTS trg_sync_${table}_${newColumn} ON ${table};`);
      contractStatements.push(`DROP FUNCTION IF EXISTS sync_${table}_${newColumn}();`);
      contractStatements.push(`ALTER TABLE ${table} DROP COLUMN IF EXISTS ${oldColumn};`);
    } else if (dialect === 'mysql') {
      expandStatements.push(`ALTER TABLE ${table} ADD COLUMN ${newColumn} ${columnType};`);
      contractStatements.push(`ALTER TABLE ${table} DROP COLUMN ${oldColumn};`);
    } else {
      expandStatements.push(`ALTER TABLE ${table} ADD COLUMN ${newColumn} ${columnType};`);
      contractStatements.push(`-- SQLite column drop requires table recreation or modern SQLite ALTER DROP`);
    }

    return {
      migrationId: `zero_downtime_${table}_${oldColumn}_to_${newColumn}`,
      phase: 'expand',
      expand: {
        statements: expandStatements,
        description: `Add new column ${newColumn} and automatic synchronization triggers for dual-writing.`,
        backwardCompatible: true,
      },
      backfill: {
        table,
        sourceColumn: oldColumn,
        targetColumn: newColumn,
        batchSize,
        throttleMs,
        strategy: 'cursor',
      },
      contract: {
        statements: contractStatements,
        prerequisites: [
          `Verify all application instances are writing to '${newColumn}'`,
          `Verify backfill completed (SELECT COUNT(*) FROM ${table} WHERE ${newColumn} IS NULL = 0)`,
          `Wait 24h grace period to ensure no rollback is required`,
        ],
        gracePeriodHours: 24,
      },
    };
  }

  /**
   * Generates batched backfill SQL script with throttle delays.
   */
  public static generateBackfillScript(plan: ZeroDowntimePlan, dialect: DatabaseDialect = 'postgres'): string {
    if (!plan.backfill) return '';
    const { table, sourceColumn, targetColumn, batchSize = 1000 } = plan.backfill;

    if (dialect === 'postgres') {
      return `
-- Zero-Downtime Batched Backfill Script
-- Updates rows in batches of ${batchSize} to prevent long-running table locks
DO $$
DECLARE
  v_rows_updated INT := 1;
BEGIN
  WHILE v_rows_updated > 0 LOOP
    WITH batch AS (
      SELECT id FROM ${table}
      WHERE ${targetColumn} IS NULL AND ${sourceColumn} IS NOT NULL
      LIMIT ${batchSize}
      FOR UPDATE SKIP LOCKED
    )
    UPDATE ${table} t
    SET ${targetColumn} = t.${sourceColumn}
    FROM batch
    WHERE t.id = batch.id;

    GET DIAGNOSTICS v_rows_updated = ROW_COUNT;
    PERFORM pg_sleep(0.05); -- 50ms throttle delay
  END LOOP;
END $$;
      `.trim();
    }

    return `
UPDATE ${table} SET ${targetColumn} = ${sourceColumn} WHERE ${targetColumn} IS NULL;
    `.trim();
  }
}

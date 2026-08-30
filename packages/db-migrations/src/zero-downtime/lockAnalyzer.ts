import { LockAnalysis, DatabaseDialect } from '../types';

export class LockAnalyzer {
  /**
   * Analyzes an array of SQL statements or a SQL script for potential locking hazards
   * and zero-downtime deployment violations.
   */
  public static analyzeStatements(sql: string, dialect: DatabaseDialect = 'postgres'): LockAnalysis[] {
    const stmts = sql
      .split(';')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);

    const analyses: LockAnalysis[] = [];

    for (const stmt of stmts) {
      analyses.push(this.analyzeStatement(stmt, dialect));
    }

    return analyses;
  }

  public static analyzeStatement(stmt: string, dialect: DatabaseDialect = 'postgres'): LockAnalysis {
    const lower = stmt.toLowerCase().replace(/\s+/g, ' ');

    // 1. ADD COLUMN NOT NULL without default or with non-constant default
    if (lower.includes('alter table') && lower.includes('add column') && lower.includes('not null') && !lower.includes('default')) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        recommendation: 'Adding NOT NULL column without DEFAULT requires full table rewrite and exclusive lock. Add as NULLABLE first, backfill values, then add NOT NULL constraint using VALIDATE CONSTRAINT.',
      };
    }

    // 2. DROP COLUMN
    if (lower.includes('alter table') && (lower.includes('drop column') || lower.includes('drop '))) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        recommendation: 'Directly dropping a column breaks running application instances that still reference it. Follow the expand/contract pattern: ignore column in code first, deploy, then drop column in contract phase.',
      };
    }

    // 3. RENAME COLUMN or TABLE
    if (lower.includes('alter table') && (lower.includes('rename column') || lower.includes('rename to'))) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        recommendation: 'Renaming columns or tables causes immediate downtime for unmigrated services. Add the new column/table, sync with triggers/dual-write, deploy updated code, then drop old column/table.',
      };
    }

    // 4. ALTER COLUMN TYPE
    if (lower.includes('alter table') && (lower.includes('alter column') || lower.includes('modify column')) && lower.includes('type')) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        recommendation: 'Changing column types requires rewriting the entire table under exclusive lock. Create a new column with the target type, dual-write, backfill asynchronously, and switch over.',
      };
    }

    // 5. CREATE INDEX without CONCURRENTLY in Postgres
    if (dialect === 'postgres' && lower.startsWith('create index') && !lower.includes('concurrently')) {
      return {
        statement: stmt,
        lockLevel: 'SHARE',
        potentialDowntime: true,
        risk: 'medium',
        recommendation: 'In PostgreSQL, creating an index without CONCURRENTLY locks the table against writes. Use "CREATE INDEX CONCURRENTLY" outside a transaction block.',
      };
    }

    // 6. DROP INDEX without CONCURRENTLY in Postgres
    if (dialect === 'postgres' && lower.startsWith('drop index') && !lower.includes('concurrently')) {
      return {
        statement: stmt,
        lockLevel: 'SHARE',
        potentialDowntime: false,
        risk: 'medium',
        recommendation: 'Use "DROP INDEX CONCURRENTLY" in PostgreSQL to avoid holding exclusive locks on parent tables.',
      };
    }

    // 7. VACUUM FULL / TRUNCATE
    if (lower.startsWith('truncate') || lower.startsWith('vacuum full')) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        recommendation: 'TRUNCATE / VACUUM FULL takes ACCESS EXCLUSIVE locks. Use batch deletion or standard vacuuming instead.',
      };
    }

    // 8. ADD COLUMN with DEFAULT (Postgres >= 11 is metadata-only, but older versions or SQLite/MySQL may lock)
    if (lower.includes('alter table') && lower.includes('add column') && lower.includes('default')) {
      return {
        statement: stmt,
        lockLevel: 'ROW_EXCLUSIVE',
        potentialDowntime: false,
        risk: 'low',
        recommendation: 'Adding a column with constant default is safe in PostgreSQL 11+, but ensure defaults are non-volatile functions.',
      };
    }

    // 9. Standard CREATE TABLE or safe DDL
    if (lower.startsWith('create table') || lower.startsWith('create view') || lower.includes('concurrently')) {
      return {
        statement: stmt,
        lockLevel: 'NONE',
        potentialDowntime: false,
        risk: 'low',
      };
    }

    return {
      statement: stmt,
      lockLevel: 'NONE',
      potentialDowntime: false,
      risk: 'low',
    };
  }
}

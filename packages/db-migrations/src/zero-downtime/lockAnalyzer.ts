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

    // 0. DROP TABLE (Destructive DDL - blocked from main)
    if (lower.startsWith('drop table') || lower.includes(' drop table ')) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        isDestructive: true,
        destructiveViolation: true,
        recommendation: 'DROP TABLE is a destructive statement causing irreversible data loss and downtime. Blocked from merging into main.',
      };
    }

    // 0b. DROP DATABASE
    if (lower.startsWith('drop database') || lower.includes(' drop database ')) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        isDestructive: true,
        destructiveViolation: true,
        recommendation: 'DROP DATABASE is completely destructive. Blocked from merging into main.',
      };
    }

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

    // 2. DROP COLUMN (Destructive)
    if (lower.includes('alter table') && (lower.includes('drop column') || lower.includes('drop '))) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        isDestructive: true,
        destructiveViolation: true,
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

    // 4. ALTER COLUMN TYPE (Destructive full rewrite)
    if (lower.includes('alter table') && (lower.includes('alter column') || lower.includes('modify column') || lower.includes('modify ')) && (lower.includes('type') || lower.includes('varchar') || lower.includes('int') || lower.includes('text') || lower.includes('bigint') || lower.includes('timestamp') || lower.includes('json'))) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        isDestructive: true,
        destructiveViolation: true,
        recommendation: 'Changing column types requires rewriting the entire table under exclusive lock and can truncate or invalidate existing data. Blocked from merging into main; use expand/contract dual-writing instead.',
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

    // 7. VACUUM FULL / TRUNCATE (Destructive)
    if (lower.startsWith('truncate') || lower.startsWith('vacuum full')) {
      return {
        statement: stmt,
        lockLevel: 'EXCLUSIVE',
        potentialDowntime: true,
        risk: 'high',
        isDestructive: true,
        destructiveViolation: true,
        recommendation: 'TRUNCATE / VACUUM FULL takes ACCESS EXCLUSIVE locks and purges data. Blocked from merging into main.',
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

  /**
   * Evaluates whether a SQL statement is classified as destructive.
   */
  public static isDestructive(statement: string): { isDestructive: boolean; reason?: string } {
    const analysis = this.analyzeStatement(statement);
    return {
      isDestructive: !!analysis.isDestructive,
      reason: analysis.recommendation,
    };
  }

  /**
   * Validates a batch or migration script against destructive SQL policies.
   */
  public static validateMigrationSafety(sql: string): { safe: boolean; violations: LockAnalysis[] } {
    const analyses = this.analyzeStatements(sql);
    const violations = analyses.filter((a) => a.destructiveViolation || a.isDestructive);
    return {
      safe: violations.length === 0,
      violations,
    };
  }
}

#!/usr/bin/env node

/**
 * Migration Safety Gate: Detect and Block Destructive SQL Statements
 *
 * Enforces policy:
 * - DROP TABLE is blocked.
 * - DROP COLUMN without expand/contract deprecation is blocked.
 * - Column type mutations (ALTER TABLE ... ALTER/MODIFY COLUMN ... TYPE) are blocked.
 * - TRUNCATE TABLE is blocked.
 */

const fs = require('fs');
const path = require('path');

const DESTRUCTIVE_RULES = [
  {
    name: 'DROP TABLE',
    regex: /\bDROP\s+TABLE\b/i,
    message: 'DROP TABLE statements cause irreversible data loss and are blocked from merging into main.',
  },
  {
    name: 'DROP DATABASE',
    regex: /\bDROP\s+DATABASE\b/i,
    message: 'DROP DATABASE statements are completely destructive and blocked.',
  },
  {
    name: 'DROP COLUMN',
    regex: /\bALTER\s+TABLE\s+[\w\.]+\s+DROP\s+(?:COLUMN\s+)?/i,
    message: 'Direct DROP COLUMN breaks running instances. Use expand/contract phased deprecation instead.',
  },
  {
    name: 'COLUMN TYPE CHANGE',
    regex: /\bALTER\s+TABLE\s+[\w\.]+\s+(?:ALTER|MODIFY)\s+(?:COLUMN\s+)?[\w\.]+\s+(?:TYPE\s+|SET\s+DATA\s+TYPE\s+|VARCHAR|INT|BIGINT|TEXT|TIMESTAMP|JSON)/i,
    message: 'Direct column type modification requires full table lock and table rewrite. Blocked from merging into main.',
  },
  {
    name: 'TRUNCATE',
    regex: /\bTRUNCATE\s+(?:TABLE\s+)?/i,
    message: 'TRUNCATE statements purge table data and are blocked from merging into main.',
  },
];

function checkSql(sql, filePath) {
  const violations = [];
  const lines = sql.split('\n');

  for (let lineNum = 0; lineNum < lines.length; lineNum++) {
    const line = lines[lineNum].trim();
    if (line.startsWith('--') || line.startsWith('//') || line.startsWith('/*')) continue;

    for (const rule of DESTRUCTIVE_RULES) {
      if (rule.regex.test(line)) {
        violations.push({
          file: filePath,
          line: lineNum + 1,
          rule: rule.name,
          statement: line,
          message: rule.message,
        });
      }
    }
  }

  return violations;
}

function scanMigrations() {
  const migrationsDir = path.resolve(__dirname, '../../packages/db-migrations/src/migrations');
  console.log(`[Safety Gate] Scanning migrations in ${migrationsDir}...`);

  if (!fs.existsSync(migrationsDir)) {
    console.warn(`[Safety Gate] Migrations directory not found at ${migrationsDir}`);
    process.exit(0);
  }

  const files = fs.readdirSync(migrationsDir).filter((f) => f.endsWith('.ts') && !f.endsWith('.d.ts'));
  const allViolations = [];

  for (const file of files) {
    const fullPath = path.join(migrationsDir, file);
    const content = fs.readFileSync(fullPath, 'utf8');

    // Only inspect the `sqlUp` property to prevent destructive statements in forward production migrations
    const sqlUpMatch = content.match(/sqlUp:\s*`([^`]+)`/s);
    if (sqlUpMatch && sqlUpMatch[1]) {
      const violations = checkSql(sqlUpMatch[1], file);
      allViolations.push(...violations);
    }
  }

  if (allViolations.length > 0) {
    console.error('\n======================================================');
    console.error('❌ MIGRATION SAFETY GATE: DESTRUCTIVE SQL DETECTED');
    console.error('======================================================');
    for (const v of allViolations) {
      console.error(`- [${v.rule}] File: ${v.file}:${v.line}`);
      console.error(`  Statement: "${v.statement}"`);
      console.error(`  Policy: ${v.message}\n`);
    }
    console.error('Destructive SQL statements are blocked from merging into main/master.');
    console.error('Please refactor the migration using expand/contract or backward-compatible DDL.');
    process.exit(1);
  } else {
    console.log('✅ [Safety Gate Passed] No destructive SQL statements found in forward migrations.');
    process.exit(0);
  }
}

if (require.main === module) {
  scanMigrations();
}

module.exports = { checkSql, DESTRUCTIVE_RULES };

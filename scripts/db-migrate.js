#!/usr/bin/env node
const { main } = require('../packages/db-migrations/dist/cli.js');
main().catch((err) => {
  console.error('[db-migrate] Error:', err);
  process.exit(1);
});

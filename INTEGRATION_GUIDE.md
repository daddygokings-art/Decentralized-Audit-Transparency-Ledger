# Regulator Audit Trail - Integration Guide

## Quick Integration Checklist

- [x] Smart contract modules (Rust)
- [x] REST API endpoints (TypeScript)
- [x] Frontend portal (React)
- [x] Compliance validators
- [x] Test suite
- [x] Documentation

## Step 1: Integrate Smart Contract Module

### Add to main contract
```rust
// In your main audit ledger contract
use crate::regulator::*;
use crate::compliance_validators::*;
use crate::tamper_evidence::*;

// When logging an event, classify it
let regulatory_class = RegulatoryEventClass {
    standard: ComplianceStandard::ISA3000,
    control_code: Symbol::new(env, "CC6.1"),
    demonstrates_control: true,
    retention_ledgers: 52560,
    sensitivity: SensitivityLevel::Confidential,
};

// Store classification with event
env.storage().persistent().set(
    &DataKey::EventRegulatoryClass(event_id),
    &regulatory_class,
);
```

## Step 2: Integrate REST API

### Add routes to Express server
```typescript
// In api/rest/src/server.ts
import { createRegulatorRoutes, regulatorAuthMiddleware } from './regulator';

// Mount regulator routes
app.use(regulatorAuthMiddleware);
app.use(createRegulatorRoutes());
```

### Update OpenAPI specification
```yaml
# In api/openapi.yaml
paths:
  /regulator/audit-trails:
    get:
      summary: Query audit trail
      security:
        - bearerAuth: []
      parameters:
        - name: startTime
          in: query
          schema:
            type: integer
        - name: endTime
          in: query
          schema:
            type: integer
      responses:
        '200':
          description: Audit trail entries
```

## Step 3: Update Frontend

### Add navigation to main layout
```tsx
// In ui/src/components/Nav.tsx
import Link from 'next/link';

export function Nav() {
  return (
    <nav>
      {/* Existing navigation */}
      <Link href="/explorer">Explorer</Link>
      
      {/* Add regulator link */}
      <Link href="/regulator" className="nav-item regulator">
        Regulator Portal
      </Link>
    </nav>
  );
}
```

### Add authentication check
```tsx
// In ui/src/lib/auth.ts
export function isRegulatorAuthenticated(): boolean {
  return !!localStorage.getItem('regulator_token');
}

export function getRegulatorContext() {
  const token = localStorage.getItem('regulator_token');
  const email = localStorage.getItem('regulator_email');
  return { token, email };
}
```

## Step 4: Configure Database (Optional)

### Create tables for DSA storage
```sql
CREATE TABLE data_sharing_agreements (
  id CHAR(64) PRIMARY KEY,
  data_provider VARCHAR(56) NOT NULL,
  regulator_address VARCHAR(56) NOT NULL,
  standards TEXT NOT NULL, -- JSON array
  allowed_event_types TEXT NOT NULL, -- JSON array
  role INTEGER NOT NULL,
  status INTEGER NOT NULL,
  active BOOLEAN DEFAULT true,
  effective_ledger INTEGER,
  expiry_ledger INTEGER,
  created_at TIMESTAMP DEFAULT NOW(),
  created_by VARCHAR(256)
);

CREATE TABLE compliance_reports (
  id CHAR(64) PRIMARY KEY,
  standard INTEGER NOT NULL,
  audit_subject VARCHAR(56) NOT NULL,
  issuer VARCHAR(256),
  generated_at TIMESTAMP,
  status INTEGER,
  events_examined INTEGER,
  controls_operating INTEGER,
  controls_deficient INTEGER,
  compliance_score INTEGER,
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_dsa_regulator ON data_sharing_agreements(regulator_address);
CREATE INDEX idx_dsa_status ON data_sharing_agreements(status);
CREATE INDEX idx_reports_audit_subject ON compliance_reports(audit_subject);
```

## Step 5: Environment Configuration

### Update .env
```bash
# Regulator Portal Configuration
REGULATOR_ENABLED=true
REGULATOR_API_KEY_REQUIRED=true
REGULATOR_JWT_SECRET="your-secret-key-here"
REGULATOR_SESSION_TIMEOUT=3600

# Database (optional)
DATABASE_URL="postgresql://user:pass@localhost/audit_db"

# Compliance Standards
ENABLE_ISA3000=true
ENABLE_SOC2=true
ENABLE_GDPR=true

# Portal Settings
PORTAL_PORT=3001
API_PORT=3002
```

## Step 6: Authentication Setup

### Create JWT verification
```typescript
// In api/rest/src/regulator.ts
import jwt from 'jsonwebtoken';

export function verifyRegulatorToken(token: string): RegulatorContext | null {
  try {
    const payload = jwt.verify(
      token,
      process.env.REGULATOR_JWT_SECRET || 'secret'
    );
    return {
      regulatorId: payload.sub,
      role: payload.role,
      standards: payload.standards,
    };
  } catch (error) {
    return null;
  }
}
```

### Create login endpoint
```typescript
app.post('/api/regulator/login', async (req, res) => {
  const { email, password } = req.body;
  
  // Validate credentials (integrate with your auth system)
  const regulator = await validateRegulator(email, password);
  
  if (!regulator) {
    return res.status(401).json({ error: 'Invalid credentials' });
  }
  
  const token = jwt.sign({
    sub: regulator.id,
    role: regulator.role,
    standards: regulator.standards,
  }, process.env.REGULATOR_JWT_SECRET || 'secret');
  
  res.json({ token });
});
```

## Step 7: Connect to Smart Contract

### RPC Integration
```typescript
// In api/rest/src/regulator.ts
import { SorobanRpc } from '@soroban-js/stellar-sdk';

const sorobanClient = new SorobanRpc.Server(
  process.env.RPC_URL || 'https://soroban-testnet.stellar.org'
);

export async function queryAuditTrail(filter: AuditTrailQuery) {
  // Call contract function to query events
  const response = await sorobanClient.getEvents({
    filters: [
      {
        type: 'contract',
        contractId: process.env.CONTRACT_ID,
        topics: [Symbol.new(env, 'audit_event')]
      }
    ],
    limit: filter.limit,
    cursor: filter.offset?.toString()
  });
  
  return response;
}
```

## Step 8: Deploy with Docker

### Update docker-compose.yml
```yaml
version: '3.8'
services:
  api:
    build: ./api/rest
    ports:
      - "3002:3002"
    environment:
      CONTRACT_ID: ${CONTRACT_ID}
      RPC_URL: ${RPC_URL}
      REGULATOR_JWT_SECRET: ${REGULATOR_JWT_SECRET}
    depends_on:
      - db

  ui:
    build: ./ui
    ports:
      - "3001:3001"
    environment:
      NEXT_PUBLIC_API_URL: http://localhost:3002

  db:
    image: postgres:15
    environment:
      POSTGRES_DB: audit_db
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./init-db.sql:/docker-entrypoint-initdb.d/init.sql
```

## Step 9: Testing Integration

### Test DSA Creation
```bash
# Deploy contract
soroban contract deploy --wasm target/wasm32-unknown-unknown/release/audit_ledger.wasm

# Test regulator API
curl -X POST http://localhost:3002/regulator/data-sharing-agreements \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "dataProvider": "GXXXXXXX...",
    "regulatorAddress": "GYYYYYYY...",
    "standards": ["ISA3000", "SOC2"],
    "role": "auditor"
  }'
```

### Test Portal Access
```bash
# Open portal in browser
open http://localhost:3001/regulator/login

# Login and verify
# - Dashboard loads
# - Can query audit trail
# - Can generate proofs
# - Can verify chains
```

## Step 10: Monitoring & Alerts

### Add logging
```typescript
// In api/rest/src/regulator.ts
import winston from 'winston';

const logger = winston.createLogger({
  level: 'info',
  format: winston.format.json(),
  transports: [
    new winston.transports.File({ filename: 'regulator-api.log' }),
    new winston.transports.Console()
  ]
});

// Log all DSA operations
logger.info('DSA created', { dsaId, regulator, standard });
logger.info('Audit trail queried', { filters, resultCount });
logger.error('Tamper evidence violation', { eventIndex });
```

### Add metrics
```typescript
// Prometheus metrics
import promClient from 'prom-client';

const auditQueriesCounter = new promClient.Counter({
  name: 'audit_queries_total',
  help: 'Total audit trail queries',
  labelNames: ['standard', 'regulator']
});

const disclosureProofsCounter = new promClient.Counter({
  name: 'disclosure_proofs_generated_total',
  help: 'Total selective disclosure proofs generated'
});

const tamperEvidenceViolations = new promClient.Gauge({
  name: 'tamper_evidence_violations',
  help: 'Number of tamper evidence violations detected'
});
```

## Maintenance

### Periodic Tasks
```bash
# Weekly: Archive old events (keep recent 1000)
0 2 * * 0 /scripts/archive_events.sh

# Daily: Run compliance checks
0 4 * * * /scripts/run_compliance_checks.sh

# Hourly: Verify chain integrity
0 * * * * /scripts/verify_chain_integrity.sh
```

### Backup Strategy
```bash
# Backup database
pg_dump audit_db > backup_$(date +%Y%m%d).sql

# Backup smart contract state
soroban contract invoke --id <contract_id> -- snapshot_state > state_backup.json

# Archive compliance reports
tar -czf reports_archive_$(date +%Y%m).tar.gz reports/
```

## Troubleshooting

### Issue: DSA not enforcing access control
**Solution**: Check DSA signature validation in `data_sharing.rs`
```rust
// Verify both signatures are present and valid
if !DSAHelper::verify_dsa_signatures(&dsa) {
    return AccessDecision::Rejected;
}
```

### Issue: Tamper evidence showing false positives
**Solution**: Ensure hash algorithm consistency
```rust
// Use same algorithm for all hash operations
let hash = env.crypto_sha256(&event_data);
```

### Issue: Selective disclosure proofs not verifying
**Solution**: Check Merkle path reconstruction
```rust
// Reconstruct root from leaf using sibling hashes
let computed_root = DisclosureHelper::reconstruct_root(
    &proof.field_hash,
    &proof.sibling_hashes,
    &proof.positions
);
```

## Next Steps

1. **Customize compliance standards**: Extend ISA3000Validator and SOC2Validator with your organization's controls
2. **Integrate with audit systems**: Connect to existing ERP/audit tools
3. **Set up dashboards**: Create custom Grafana dashboards for compliance metrics
4. **Train regulators**: Provide documentation and training for portal users
5. **Establish SLAs**: Define response times for audit queries and report generation

## Support Resources

- Smart Contract API: `src/regulator.rs` line comments
- REST API: `api/rest/src/regulator.ts` JSDoc comments
- Portal Code: `ui/src/app/regulator/` component documentation
- Tests: `src/regulator_tests.rs` for usage examples
- Docs: `docs/regulator-audit-trails.md` for detailed specifications


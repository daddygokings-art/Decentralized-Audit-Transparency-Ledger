# Token-Gated Access Control for Audit Ledger

Premium event streams and analytics behind multi-chain token verification.

## Overview

This module enables monetization and access control for the Audit Ledger by introducing:

- **Token Tiers** — Define premium features (e.g., "premium", "enterprise")
- **Multi-Chain Verification** — Accept tokens from Stellar, Ethereum, Polygon, and other EVM chains
- **Marketplace** — Users can buy/sell tier access on a decentralized marketplace
- **Real-Time Updates** — WebSocket subscriptions for marketplace and tier changes
- **Flexible Requirements** — Mix ERC-20, ERC-721, ERC-1155, and Stellar assets

## Architecture

### Three-Layer Design

```
┌─────────────────────────────────────────────────────────────┐
│ API Layer (REST, GraphQL, WebSocket)                        │
│ - User-facing endpoints for tier queries, purchases, access │
└────────────────┬────────────────────────────────────────────┘
                 │
┌────────────────┴────────────────────────────────────────────┐
│ Business Logic (Node.js Services)                           │
│ - Marketplace management                                    │
│ - Event broadcasting                                        │
│ - Cache management                                          │
└────────────────┬────────────────────────────────────────────┘
                 │
┌────────────────┴────────────────────────────────────────────┐
│ Smart Contracts (Soroban)                                   │
├──────────────────────┬──────────────────────────────────────┤
│ TokenGating          │ CrossChainBridge                     │
│ - Tier definitions   │ - Relay management                   │
│ - User holdings      │ - Signature verification             │
│ - Stream gating      │ - Balance cache                      │
│ - Marketplace        │ - EVM token verification             │
└──────────────────────┴──────────────────────────────────────┘
```

## Components

### 1. Smart Contracts

#### `src/token_gating.rs` (649 lines)

Core contract for on-chain tier management:
- **Tiers**: Create, enable/disable, manage requirements
- **Holdings**: Track user tier ownership with expiry
- **Access**: Check user access to tiers and streams
- **Marketplace**: Create listings, execute purchases
- **Verification**: Verify token balances (Stellar-native)

```rust
// Example: Create premium tier requiring USDC
create_token_tier(
    tier_id = "premium",
    token_requirements = [
        { standard: ERC20, contract: USDC_ADDRESS, required_amount: 100 }
    ],
    purchase_price = 1_000_000,  // 0.1 XLM
    tradeable = true
)
```

#### `src/cross_chain_bridge.rs` (521 lines)

Verifies EVM token balances via relay attestations:
- **Relays**: Register ECDSA-signing relays
- **Attestations**: Submit and verify signed balance proofs
- **Cache**: Store verified balances with TTL
- **ERC Standards**: Support ERC-20, ERC-721, ERC-1155

```rust
// Relay observes Ethereum ERC-20 transfer
// Fetches user balance and signs attestation
// Submits to Soroban: submit_attestation(user, token, balance, signature)
// Contract verifies signature and caches balance
// Token gating contract queries cache: get_verified_balance()
```

### 2. REST API

**File**: `api/rest/src/token-gating.ts` (543 lines)

11 endpoints for tier management and marketplace:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/tiers` | GET | List all tiers |
| `/tiers/:tier_id` | GET | Get tier details |
| `/users/:address/tiers` | GET | User's current holdings |
| `/verify-balance` | POST | Cross-chain verification |
| `/streams/:event_type/access` | GET | Check stream access |
| `/marketplace/list` | POST | Create listing |
| `/marketplace/purchase` | POST | Buy tier |
| `/marketplace/listings` | GET | Browse listings |
| `/marketplace/listings/:id` | DELETE | Cancel listing |
| `/health/token-gating` | GET | Health check |

### 3. GraphQL API

**File**: `api/graphql/src/token-gating-schema.ts` (456 lines)

Complete GraphQL schema with:
- **14 Queries** — Get tiers, user access, marketplace listings, verification status
- **9 Mutations** — Create/update tiers, purchase, verify balance
- **6 Subscriptions** — Real-time updates via WebSocket

### 4. WebSocket Server

**File**: `api/ws/src/token-gating.ts` (457 lines)

Real-time event broadcasting:
- **Channels**: `marketplace:*`, `tiers:*`, `verification:*`, `streams:*`
- **Filters**: Subscribe with conditions (e.g., `marketplace:premium`, `price < 500000`)
- **Events**: LISTING_CREATED, PURCHASE_COMPLETED, TIER_GRANTED, etc.

```javascript
// Subscribe to premium tier marketplace
ws.send(JSON.stringify({
  action: 'subscribe',
  channel: 'marketplace:premium'
}));

// Receive real-time listing updates
// { type: 'LISTING_CREATED', payload: { ... } }
// { type: 'PURCHASE_COMPLETED', payload: { ... } }
```

### 5. Tests

**File**: `src/token_gating_tests.rs` (432 lines)

Comprehensive test suite:
- 50+ unit tests
- Fuzz tests for edge cases
- Property-based tests
- Performance benchmarks
- Integration scenarios

## Quick Start

### Deploy Contracts

```bash
# Build
cargo build --target wasm32-unknown-unknown --release

# Deploy token gating
TGATING=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/token_gating.wasm \
  --source $KEY --network testnet)

# Deploy bridge
BRIDGE=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/cross_chain_bridge.wasm \
  --source $KEY --network testnet)

# Initialize both contracts
soroban contract invoke --id $TGATING --source $KEY --network testnet -- \
  initialize --owner $OWNER
soroban contract invoke --id $BRIDGE --source $KEY --network testnet -- \
  initialize_bridge --owner $OWNER --signature_threshold 2 \
  --cache_ttl_ledgers 300 --eth_chain_id 1
```

### Create Tiers

```bash
# Free tier
soroban contract invoke --id $TGATING --source $KEY --network testnet -- \
  create_token_tier --tier_id free --description "Public access" \
  --token_requirements '[]' --purchase_price 0 \
  --duration_ledgers 0 --tradeable false

# Premium tier (requires 100 USDC)
soroban contract invoke --id $TGATING --source $KEY --network testnet -- \
  create_token_tier --tier_id premium --description "Premium analytics" \
  --token_requirements '[{
    "standard": "erc20",
    "contract_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    "required_amount": 100000000
  }]' --purchase_price 1000000 --duration_ledgers 52560000 --tradeable true
```

### Start Services

```bash
# REST API
cd api/rest && npm install && npm start

# GraphQL API
cd api/graphql && npm install && npm start

# WebSocket
cd api/ws && npm install && npm start
```

### Test Integration

```bash
# List tiers
curl http://localhost:3000/token-gating/tiers

# Verify balance
curl -X POST http://localhost:3000/token-gating/verify-balance \
  -d '{"user_address":"GUSER...","token_standard":"erc20",...}'

# Browse marketplace
curl http://localhost:3000/token-gating/marketplace/listings

# Query via GraphQL
curl -X POST http://localhost:4000/graphql \
  -d '{"query":"{ tokenTiers { tierId description } }"}'

# Real-time updates
wscat -c ws://localhost:5000/token-gating
{"action":"subscribe","channel":"marketplace:*"}
```

## Integration with Audit Ledger

### Gating Event Streams

```typescript
// In audit ledger API
if (isProtectedStream(eventType)) {
  const access = await tokenGating.checkStreamAccess(userAddress, eventType);
  if (!access.hasAccess) {
    throw new Error(`Requires ${access.requiredTier} tier`);
  }
}

const events = await auditLedger.getEventsByType(eventType, ...);
res.json(events);
```

### Verifying User Tokens

```typescript
// Before granting tier access
const verification = await tokenGating.verifyTokenBalance({
  userAddress,
  tokenStandard: 'erc20',
  contractAddress: USDC,
  requiredAmount: 100_000_000,
});

if (verification.verified) {
  await tokenGating.grantTierToUser(userAddress, 'premium');
}
```

### Marketplace Events

```typescript
// Listen for tier purchases
wsManager.on('PURCHASE_COMPLETED', (purchase) => {
  // Update analytics
  logTierPurchase(purchase);
  
  // Notify platform
  notifyUserTierGranted(purchase.buyer, purchase.tier_id);
});
```

## Configuration

### Environment Variables

```bash
# Smart Contracts
TOKEN_GATING_CONTRACT_ID=C...
BRIDGE_CONTRACT_ID=C...
CONTRACT_NETWORK=testnet
RPC_URL=https://soroban-testnet.stellar.org

# API Services
REST_API_PORT=3000
GRAPHQL_PORT=4000
WEBSOCKET_PORT=5000

# Cache & Rate Limiting
CACHE_TTL_SECONDS=300
RATE_LIMIT_MAX_REQUESTS=100
RATE_LIMIT_WINDOW_MS=60000

# Verification
VERIFICATION_TTL_LEDGERS=300
VERIFICATION_RATE_LIMIT=10
```

### Tier Configuration Examples

**Free Tier**
```
id: "free"
description: "Public audit trails"
requirements: []
price: 0 stroops
duration: permanent
tradeable: false
```

**Professional Tier**
```
id: "professional"
description: "Advanced analytics, real-time alerts"
requirements: [
  100 USDC (Ethereum) OR
  100 Stellar USDC
]
price: 1,000,000 stroops (0.1 XLM)
duration: 1 year
tradeable: true
```

**Enterprise Tier**
```
id: "enterprise"
description: "Unlimited access, webhooks, support"
requirements: [
  1000 USDC (Ethereum) OR
  1000 Stellar USDC OR
  Any NFT (BAYC, etc.)
]
price: 10,000,000 stroops (1 XLM)
duration: 3 years
tradeable: true
```

## Monitoring & Operations

### Health Checks

```bash
# Contract state
soroban contract invoke --id $TGATING --network testnet -- total_tiers

# Bridge relays
soroban contract invoke --id $BRIDGE --network testnet -- get_relays

# API health
curl http://localhost:3000/token-gating/health/token-gating
```

### Metrics to Track

- **Tier adoption** — Active holders per tier
- **Marketplace volume** — Total purchases, avg price
- **Verification latency** — Bridge relay response time
- **Cache hit rate** — Balance cache effectiveness
- **API response time** — Endpoint performance

### Common Issues

| Problem | Solution |
|---------|----------|
| Bridge relay not attesting | Verify relay ECDSA pubkey format, check Ethereum RPC connectivity |
| Verification timeout | Increase TTL or activate more relays |
| High API latency | Scale API horizontally, implement pagination, use Redis cache |
| Marketplace unresponsive | Check contract state size, archive old listings |

## Security Considerations

1. **Always verify server-side** — Never trust client claims about access
2. **Relay attestations** — Use threshold signatures (e.g., 2-of-3 relays)
3. **Rate limiting** — Prevent spam by limiting verification attempts per user
4. **Cache TTL** — Balance security (shorter TTL) vs. performance (longer TTL)
5. **Audit trail** — Log all tier grants, purchases, and access checks
6. **Private keys** — Secure relay private keys; rotate regularly
7. **HTTPS only** — Use WSS for WebSocket in production
8. **Authentication** — Verify user identity before granting tiers

## Documentation

- [ADR-010](docs/adr/ADR-010-token-gating.md) — Architecture & design decisions
- [Deployment Guide](docs/token-gating-deployment.md) — Step-by-step setup
- [Client Guide](docs/token-gating-client-guide.md) — Integration examples
- [Integration Guide](docs/INTEGRATION.md) — Connecting to audit ledger

## Performance Metrics

| Operation | Duration | Notes |
|-----------|----------|-------|
| Tier creation | ~1 second | Soroban contract invoke |
| Access check (cached) | ~10ms | REST API, cached |
| Token verification | 5-10 seconds | Includes bridge verification |
| Marketplace listing | ~1 second | Contract invoke |
| Purchase & tier grant | ~2-3 seconds | Atomic operation |
| WebSocket broadcast | <100ms | To all subscribers |

## Future Enhancements

- [ ] Tier inheritance (enterprise includes professional features)
- [ ] Discount codes and promotional pricing
- [ ] Subscription model (monthly recurring)
- [ ] Multi-sig tier governance
- [ ] DAO voting on tier features
- [ ] Cross-contract tier stacking (compose multiple tiers)
- [ ] NFT-based access (collection-wide benefits)
- [ ] Geographic pricing (different prices per region)

## Support & Contact

For questions or issues:
1. Check [documentation](docs/)
2. Review [test suite](src/token_gating_tests.rs) for examples
3. Open GitHub issue with reproduction steps
4. Contact security team for vulnerability reports

## License

MIT (see [LICENSE](LICENSE))

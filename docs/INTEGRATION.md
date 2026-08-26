# Token Gating Integration Guide

Quick reference for integrating token-gated access with the existing Audit Ledger.

## File Structure

```
src/
├── lib.rs                              # Main audit ledger contract
├── token_gating.rs              (NEW) # Token gating contract
├── cross_chain_bridge.rs        (NEW) # EVM bridge for verification
└── token_gating_tests.rs        (NEW) # Comprehensive test suite

api/
├── rest/src/
│   └── token-gating.ts          (NEW) # REST API endpoints
├── graphql/src/
│   └── token-gating-schema.ts   (NEW) # GraphQL types & resolvers
└── ws/src/
    └── token-gating.ts          (NEW) # WebSocket real-time events

docs/
├── adr/
│   └── ADR-010-token-gating.md  (NEW) # Architecture decision record
├── token-gating-deployment.md   (NEW) # Deployment guide
└── token-gating-client-guide.md (NEW) # Client integration examples
```

## Quick Start

### 1. Deploy Contracts

```bash
# Build both contracts
cargo build --target wasm32-unknown-unknown --release

# Deploy token gating
TGATING=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/token_gating.wasm \
  --source <key> --network testnet)

# Deploy bridge
BRIDGE=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/cross_chain_bridge.wasm \
  --source <key> --network testnet)

# Initialize
soroban contract invoke --id $TGATING --source <key> --network testnet -- \
  initialize --owner <owner_addr>

soroban contract invoke --id $BRIDGE --source <key> --network testnet -- \
  initialize_bridge --owner <owner_addr> --signature_threshold 2 \
  --cache_ttl_ledgers 300 --eth_chain_id 1
```

### 2. Create Tiers

```bash
# Create free tier
soroban contract invoke --id $TGATING --source <key> --network testnet -- \
  create_token_tier --tier_id free --description "Public access" \
  --token_requirements '[]' --purchase_price 0 \
  --duration_ledgers 0 --tradeable false

# Create premium tier (requires ERC-20)
soroban contract invoke --id $TGATING --source <key> --network testnet -- \
  create_token_tier --tier_id premium \
  --description "Premium analytics" \
  --token_requirements '[{
    "standard": "erc20",
    "contract_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    "token_id": 0,
    "required_amount": 100000000
  }]' \
  --purchase_price 1000000 \
  --duration_ledgers 52560000 \
  --tradeable true
```

### 3. Gating Audit Streams

In your existing audit ledger contract or API:

```rust
// Pseudo-code: gating premium streams
fn log_event_with_gating(
    env: Env,
    submitter: Address,
    event_type: Symbol,
    metadata: Bytes,
    user_address: Address,
) -> Result<u32, ContractError> {
    // Check token gating access
    if event_type == Symbol::new(&env, "premium-analytics") {
        let has_access = token_gating::can_access_stream(
            &env,
            user_address,
            Symbol::new(&env, "premium"),
        );
        
        if !has_access {
            return Err(ContractError::AccessDenied);
        }
    }
    
    // Log event normally
    log_event(&env, submitter, event_type, metadata)
}
```

### 4. Wire Up APIs

In your main Express/GraphQL server:

```typescript
import express from 'express';
import tokenGatingRouter from './api/rest/src/token-gating';
import tokenGatingTypeDefs from './api/graphql/src/token-gating-schema';
import { setupTokenGatingWebSocket } from './api/ws/src/token-gating';

const app = express();
const server = createServer(app);

// REST endpoints
app.use('/token-gating', tokenGatingRouter);

// GraphQL
const schema = buildSchema(`
  ${baseTypeDefs}
  ${tokenGatingTypeDefs}
`);

// WebSocket
const wsManager = setupTokenGatingWebSocket(server);

server.listen(3000);
```

## Key Integration Points

### 1. Event Stream Gating

**Before**: Any user could query events
**After**: Check `can_access_stream()` before returning premium events

```typescript
app.get('/events', async (req, res) => {
  const { event_type, user_address } = req.query;
  
  // Check access for premium streams
  const hasAccess = await tokenGatingClient.checkStreamAccess(
    user_address,
    event_type
  );
  
  if (!hasAccess) {
    return res.status(403).json({ 
      error: `Requires ${hasAccess.required_tier} tier` 
    });
  }
  
  // Fetch events
  const events = await auditLedger.getEventsByType(event_type);
  res.json(events);
});
```

### 2. Real-Time Verification

When user submits proof of token ownership:

```typescript
// REST endpoint
app.post('/verify-and-access', async (req, res) => {
  const { user_address, token_standard, contract_address } = req.body;
  
  // Verify on-chain
  const verification = await tokenGatingClient.verifyTokenBalance({
    user_address,
    token_standard,
    contract_address,
    required_amount: 100_000_000,
  });
  
  if (verification.verified) {
    // Grant tier access
    await tokenGatingClient.grantTierToUser(
      user_address,
      'premium'
    );
    
    // Broadcast WebSocket event
    wsManager.broadcastTierEvent('granted', {
      holder: user_address,
      tier_id: 'premium',
      expiry_ledger: 0,
    });
  }
  
  res.json(verification);
});
```

### 3. Marketplace Integration

Allow users to buy/sell tier access:

```typescript
// Create listing
app.post('/marketplace/sell', async (req, res) => {
  const { tier_id, price } = req.body;
  const seller = req.user.address;
  
  const listing = await tokenGatingClient.createListing({
    tier_id,
    price,
    quantity: 0, // unlimited
  });
  
  res.json(listing);
});

// Purchase
app.post('/marketplace/buy', async (req, res) => {
  const { listing_id } = req.body;
  const buyer = req.user.address;
  
  const purchase = await tokenGatingClient.purchaseFromMarketplace({
    listing_id,
  });
  
  if (purchase.success) {
    // Broadcast to all listening clients
    wsManager.broadcastMarketplaceEvent('purchased', purchase);
  }
  
  res.json(purchase);
});
```

### 4. WebSocket Live Updates

Connect client to real-time updates:

```typescript
// Client-side
const wsClient = new WebSocket('ws://localhost:5000/token-gating');

// Subscribe to user's tier changes
wsClient.send(JSON.stringify({
  action: 'subscribe',
  channel: `tiers:${userAddress}`,
}));

// Listen for updates
wsClient.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.type === 'TIER_GRANTED') {
    // Update UI: user now has access
    updateUserAccess(message.payload.tier_id);
  }
};
```

## Deployment Checklist

- [ ] Deploy `token_gating.rs` contract
- [ ] Deploy `cross_chain_bridge.rs` contract
- [ ] Register 2+ bridge relays (for threshold signatures)
- [ ] Create initial tiers (free, premium, enterprise)
- [ ] Set stream access controls
- [ ] Integrate REST endpoints
- [ ] Add GraphQL schema and resolvers
- [ ] Start WebSocket server
- [ ] Test tier verification flow (end-to-end)
- [ ] Monitor bridge relay attestations
- [ ] Load test marketplace with synthetic purchases
- [ ] Set up monitoring/alerting

## Testing Flow

```bash
# 1. Verify contracts deployed
soroban contract invoke --id $TGATING --network testnet -- total_tiers
soroban contract invoke --id $BRIDGE --network testnet -- get_bridge_config_info

# 2. Create test tier
soroban contract invoke --id $TGATING --network testnet -- \
  create_token_tier --tier_id test --description "Test" ...

# 3. Test token verification
curl -X POST http://localhost:3000/token-gating/verify-balance \
  -d '{"user_address":"...", "token_standard":"erc20", ...}'

# 4. Check marketplace
curl http://localhost:3000/token-gating/marketplace/listings

# 5. Monitor WebSocket
wscat -c ws://localhost:5000/token-gating
{"action":"subscribe","channel":"marketplace:test"}
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Bridge not accepting attestations | Check relay signature format (65 bytes), verify ECDSA pubkey |
| Marketplace listing not showing | Verify tier is enabled, check listing is active (not cancelled) |
| WebSocket not broadcasting | Confirm subscription channel matches event channel, check ws server logs |
| Token verification times out | Check bridge relay connectivity, increase cache_ttl_ledgers |
| High API latency | Implement pagination for listings, use Redis cache, scale API horizontally |

## Configuration Reference

**Token Gating Contract**
- `global_max_tiers`: Max tier definitions (default: unlimited)
- `verification_rate_limit`: Max verifications per ledger per user (default: 10)
- `stream_gating`: Per-event-type access requirements

**Bridge Contract**
- `signature_threshold`: Min relays for acceptance (e.g., 2-of-3)
- `cache_ttl_ledgers`: Verification cache duration (~300 ledgers ≈ 1 hour)
- `max_block_age`: Max Ethereum block age (256 blocks ≈ 1 hour)

**REST API**
- `RATE_LIMIT_MAX_REQUESTS`: Default 100 per window
- `RATE_LIMIT_WINDOW_MS`: Default 60000 (1 minute)
- `CACHE_TTL_SECONDS`: Default 300 (5 minutes)

**WebSocket**
- `WS_HEARTBEAT_INTERVAL`: Keep-alive interval (default: 30s)
- `WS_MAX_CONCURRENT_SUBSCRIPTIONS`: Per-connection limit (default: 50)

## Performance Metrics

- **Tier creation**: ~1 second (Soroban invocation)
- **Access verification (cached)**: ~10ms (REST API)
- **Token balance verification**: ~5-10 seconds (includes bridge verification)
- **Marketplace listing**: ~1 second (contract invocation)
- **Purchase execution**: ~2-3 seconds (contract invocation + tier grant)
- **WebSocket broadcast**: <100ms (to all subscribers)

## Security Notes

1. **Always verify server-side** — Never trust client claims about tier access
2. **Signature verification** — Bridge relays must sign attestations; validate in contract
3. **Rate limiting** — Prevent verification spam with per-user caps
4. **Cache TTL** — Shorter TTL = more security but higher load
5. **Relay reputation** — Monitor relay performance; deactivate dishonest relays
6. **Access tokens** — Use JWT or similar for API authentication
7. **HTTPS only** — Encrypt WebSocket connections (WSS in production)

## Next Steps

1. Read [ADR-010](docs/adr/ADR-010-token-gating.md) for architectural details
2. Follow [Deployment Guide](docs/token-gating-deployment.md) for step-by-step setup
3. Review [Client Guide](docs/token-gating-client-guide.md) for integration examples
4. Run test suite: `cargo test`
5. Deploy to testnet
6. Load test with synthetic marketplace activity
7. Perform security audit
8. Deploy to mainnet

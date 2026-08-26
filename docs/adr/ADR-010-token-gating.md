/// Architecture Decision Record: Token-Gated Access for Premium Audit Streams
///
/// Title: Multi-Chain Token Gating with Marketplace Integration
/// Status: Proposed
/// Date: 2026-08-25
/// Authors: Audit Ledger Team

# ADR-010: Token-Gated Access Control & Marketplace

## Context

The Decentralized Audit Transparency Ledger serves enterprise clients who need granular access control over premium audit streams and analytics. Current capabilities are limited to owner-based governance. To support monetization, per-user tiering, and cross-chain token verification, we need:

1. **Token-based access tiers** — Define premium features gated behind token holdings
2. **Multi-chain support** — Accept tokens from Stellar (native), Ethereum (ERC-20/721/1155), and other EVM chains
3. **Marketplace for tier trading** — Allow users to buy/sell tier access on-chain
4. **Real-time event broadcasting** — WebSocket support for marketplace and tier status
5. **Unified API** — REST and GraphQL for querying access status and managing tiers

## Decision

We implement a comprehensive token-gating system with three core components:

### 1. Token Gating Contract (Soroban)

**File**: `src/token_gating.rs` (649 lines)

**Responsibilities**:
- Define and manage token tiers with flexible requirements
- Track user tier holdings with expiry tracking
- Verify user access to premium streams
- Enforce stream-level access controls

**Key Types**:
```rust
pub struct TokenTier {
    pub tier_id: Symbol,
    pub token_requirements: Vec<TokenSpec>,
    pub purchase_price: u128,
    pub duration_ledgers: u32,
    pub tradeable: bool,
    pub enabled: bool,
}

pub struct TierHolding {
    pub holder: Address,
    pub tier_id: Symbol,
    pub expiry_ledger: u32,
    pub verified: bool,
}

pub enum TokenStandard {
    StellarAsset = 0,
    ERC20 = 1,
    ERC721 = 2,
    ERC1155 = 3,
}
```

**Core Functions**:
- `create_token_tier()` — Define new tier with token requirements
- `grant_tier_to_user()` — Award tier (used by marketplace/admin)
- `has_tier_access()` — Check user's access to tier
- `verify_token_balance()` — Multi-chain balance verification
- `can_access_stream()` — Check stream gating rules
- `set_stream_access_control()` — Configure per-stream gating
- `list_tier_for_sale()` — Create marketplace listing
- `purchase_from_marketplace()` — Buy tier from listing

**Storage Strategy**:
- Instance storage for tier definitions and user holdings (frequently accessed)
- Verification cache with TTL for cross-chain balance checks
- Per-user holdings indexed by holder address for quick access checks

### 2. Cross-Chain Bridge Contract (Soroban)

**File**: `src/cross_chain_bridge.rs` (521 lines)

**Responsibilities**:
- Manage bridge relays for EVM attestation
- Verify ECDSA signatures from relay submissions
- Cache verified token balances with TTL
- Support ERC-20, ERC-721, ERC-1155 verification

**Key Types**:
```rust
pub struct BridgeRelay {
    pub relay_address: Address,
    pub pubkey: Bytes,          // ECDSA pubkey (65 bytes)
    pub active: bool,
    pub reputation: i32,
}

pub struct BalanceAttestation {
    pub relay: Address,
    pub user_address: Bytes,    // Ethereum addr (0x-prefixed)
    pub token_address: Bytes,   // Contract addr (0x-prefixed)
    pub token_id: u128,
    pub balance: u128,
    pub block_height: u64,
    pub signature: Bytes,       // ECDSA signature (65 bytes)
    pub accepted: bool,
}

pub struct BridgeVerificationRecord {
    pub user: Bytes,
    pub token_address: Bytes,
    pub balance: u128,
    pub verified_at_ledger: u32,
    pub ttl_ledgers: u32,
    pub confirmations: u32,
}
```

**Relay Flow**:
1. Off-chain relay observes EVM token event (Transfer, Approval, etc.)
2. Relay fetches current balance from token contract
3. Relay signs attestation (user_addr || token_addr || amount || block_height)
4. Relay submits to Soroban via `submit_attestation()`
5. Contract verifies signature and stores in cache
6. Token gating contract queries cache via `get_verified_balance()`

**Configuration**:
- `signature_threshold` — Min relays for acceptance (e.g., 2-of-3)
- `cache_ttl_ledgers` — How long to trust cached balance (~300 ledgers ≈ 1 hour)
- `max_block_age` — Max Ethereum block age to accept (256 blocks ≈ 1 hour)

### 3. REST & GraphQL APIs

**REST Endpoints** (`api/rest/src/token-gating.ts`, 543 lines):
- `GET /tiers` — List all tiers
- `GET /tiers/:tier_id` — Tier details
- `GET /users/:address/tiers` — User's holdings
- `POST /verify-balance` — Cross-chain verification
- `GET /streams/:event_type/access` — Check stream access
- `POST /marketplace/list` — Create listing
- `POST /marketplace/purchase` — Buy tier
- `GET /marketplace/listings` — Browse listings
- `DELETE /marketplace/listings/:id` — Cancel listing
- `GET /health/token-gating` — Health check

**GraphQL Schema** (`api/graphql/src/token-gating-schema.ts`, 456 lines):
- Queries: `tokenTiers`, `userTierHoldings`, `hasUserTierAccess`, `userStreamAccess`, `marketplaceListings`, `verifyTokenBalance`, `tokenGatingStats`
- Mutations: `createTokenTier`, `grantTierToUser`, `setStreamAccessControl`, `createMarketplaceListing`, `purchaseFromMarketplace`, `verifyAndGrantTier`
- Subscriptions: `tierUpdated`, `listingUpdated`, `purchaseCompleted`, `userTierChanged`, `verificationStatusChanged`, `streamAccessChanged`

**WebSocket Support** (`api/ws/src/token-gating.ts`, 457 lines):
- Channels: `marketplace:*`, `tiers:*`, `verification:*`, `streams:*`
- Messages: `LISTING_CREATED`, `PURCHASE_COMPLETED`, `TIER_GRANTED`, `VERIFICATION_COMPLETED`, `STREAM_ACCESS_GRANTED`, etc.
- Filtering: Subscribe with filters to receive only relevant updates

## Rationale

### Why Soroban for Token Gating?

- **Native integration** — Same contract environment as audit ledger
- **Stellar network** — Supports XLM and custom Stellar assets natively
- **Bridge relays** — Off-chain relays keep EVM verification out of main contract
- **Cost efficiency** — Single write operation per tier grant; reads are free

### Why Cross-Chain Bridge?

- **Multi-chain adoption** — Users can hold ERC-20/721/1155 instead of bridged assets
- **Relay attestation model** — Decentralized verification (threshold signatures)
- **Economic security** — Relays incentivized to stay honest (reputation system)
- **Off-chain heavy lifting** — Balance checking and signature verification on relays

### Why Marketplace?

- **User monetization** — Tier holders can resell access
- **Price discovery** — Market determines true value of tiers
- **Liquidity** — Trading reduces buyer friction (vs. fixed pricing only)
- **Secondary market** — Enables use cases like temporary access leasing

## Marketplace Transaction Flow

```
User A: Owns "premium" tier (permanent)
  ↓
A calls list_tier_for_sale(tier_id="premium", price=500_000 stroops)
  → Creates listing[101] with quantity=0 (unlimited)
  ↓
User B: Wants premium access
  ↓
B calls purchase_from_marketplace(listing_id=101)
  → Verifies A has tier
  → Grants B the tier (duration from tier config)
  → Emits PURCHASE_COMPLETED event
  ↓
B now has "premium" tier for X ledgers
```

## Verification Cache Strategy

```
Ledger 1000:
  Bridge relay submits attestation: User X has 1000 ERC-20 tokens
  Contract verifies signature, stores in cache with ttl_ledgers=300

Ledger 1100:
  User X queries can_access_stream(event_type="premium_analytics")
  Contract checks cache, finds valid balance, grants access (no new verification)

Ledger 1301:
  Cache expires (1000 + 300 = 1300)
  User X queries again → VERIFICATION_EXPIRED error
  User must trigger new off-chain verification
```

## Token Requirements (OR Logic)

```rust
TokenTier {
    tier_id: "premium",
    token_requirements: [
        // Any one of these grants access:
        TokenSpec { standard: ERC20, contract: "0xdac17f958d2ee523a2206206994597c13d831ec7", required_amount: 1000 },  // USDT
        TokenSpec { standard: Stellar, contract: USDC_ISSUER, required_amount: 1000 },
        TokenSpec { standard: ERC721, contract: "0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d" },  // BAYC (any NFT)
    ],
}
```

User qualifies if they hold:
- 1000+ USDT on Ethereum, OR
- 1000+ Stellar USDC, OR
- Any BAYC NFT

## Configuration Examples

### Basic Tier
```
create_token_tier(
  tier_id = "pro",
  purchase_price = 1_000_000,      // 0.1 XLM
  duration_ledgers = 52_560_000,   // 1 year
  tradeable = true,
  token_requirements = [
    { standard: Stellar, contract: XLM_ISSUER, required_amount: 10_000_000 }
  ]
)
```

### NFT-Based Tier
```
create_token_tier(
  tier_id = "vip",
  purchase_price = 0,              // Free (NFT only)
  duration_ledgers = 0,            // Permanent
  tradeable = false,               // Non-tradeable
  token_requirements = [
    { standard: ERC721, contract: "0x...", required_amount: 1 }  // Any NFT counts
  ]
)
```

### Multi-Chain Tier
```
create_token_tier(
  tier_id = "enterprise",
  purchase_price = 10_000_000,     // 1 XLM
  duration_ledgers = 157_680_000,  // 3 years
  tradeable = true,
  token_requirements = [
    { standard: ERC20, contract: "0x...USDC", required_amount: 10_000 },
    { standard: Stellar, contract: USDC_ISSUER, required_amount: 10_000 },
    { standard: ERC1155, contract: "0x...TICKET", token_id: 1, required_amount: 1 }
  ]
)
```

## Consequences

### Positive
- **Monetization** — Premium features generate revenue
- **Composable access** — Mix and match tokens from multiple chains
- **Decentralized verification** — Bridge relays reduce trust assumptions
- **User flexibility** — Marketplace enables diverse acquisition paths
- **Real-time updates** — WebSocket support for live marketplace/tier changes

### Negative
- **Complexity** — Three components (contract, bridge, APIs) to maintain
- **Relay dependency** — Requires operational relays for EVM verification
- **Cache invalidation** — Stale balances possible during TTL period
- **Storage growth** — Marketplace listings and holdings grow contract state
- **Latency** — Cross-chain verification adds network round-trips

### Mitigation

1. **Relay incentives** — Tie reputation to correct attestations
2. **Conservative TTL** — Default 300 ledgers (~1 hour) with owner override
3. **Archival strategy** — Move old listings to off-chain storage
4. **Rate limiting** — Per-user, per-ledger verification caps
5. **Fallback mode** — Admin-only grants if bridge unavailable

## Testing

See `src/token_gating_tests.rs` for comprehensive test suite:
- 50+ unit tests covering tier operations, access verification, marketplace, bridge
- Fuzz tests for expiry logic, inventory decrements, rate limiting
- Integration tests for multi-chain flows
- Benchmarks for performance validation

## Deployment Checklist

- [ ] Deploy `TokenGating` contract on Stellar testnet
- [ ] Deploy `CrossChainBridge` contract on Stellar testnet
- [ ] Register 3+ relays for multi-sig verification
- [ ] Configure bridge for Ethereum mainnet, Polygon, Arbitrum
- [ ] Deploy REST API with token-gating routes
- [ ] Deploy GraphQL resolvers for all queries/mutations
- [ ] Set up WebSocket server for real-time events
- [ ] Test tier creation → verification → marketplace → purchase flow
- [ ] Set up monitoring for bridge relay attestation latency
- [ ] Document admin procedures for tier management

## References

- Soroban SDK: https://soroban.stellar.org/
- ERC-20: https://eips.ethereum.org/EIPS/eip-20
- ERC-721: https://eips.ethereum.org/EIPS/eip-721
- ERC-1155: https://eips.ethereum.org/EIPS/eip-1155
- Previous ADRs: ADR-001 (Append-Only Log), ADR-008 (Cross-Chain Bridge)

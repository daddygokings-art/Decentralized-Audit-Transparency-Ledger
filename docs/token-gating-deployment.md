# Token Gating Deployment Guide

Complete walkthrough for deploying and configuring the token-gated access system for premium audit streams.

## Prerequisites

- Rust toolchain with WASM target: `rustup target add wasm32-unknown-unknown`
- Soroban CLI: `cargo install soroban-cli --features opt`
- Node.js 16+ for REST/GraphQL/WebSocket services
- Docker & Docker Compose
- Access to Stellar testnet/mainnet RPC endpoint
- Bridge relay setup for EVM chains (Ethereum, Polygon, etc.)

## Phase 1: Smart Contract Deployment

### 1. Build Contracts

```bash
cd /workspaces/Decentralized-Audit-Transparency-Ledger

# Build token gating contract
cargo build --target wasm32-unknown-unknown --release
ls -lh target/wasm32-unknown-unknown/release/token_gating.wasm

# Build cross-chain bridge contract
cargo build --target wasm32-unknown-unknown --release
ls -lh target/wasm32-unknown-unknown/release/cross_chain_bridge.wasm
```

### 2. Deploy Token Gating Contract

```bash
export SOROBAN_SECRET_KEY="<your_deployer_secret_key>"
export NETWORK="testnet"  # or "mainnet"
export RPC_URL="https://soroban-testnet.stellar.org"

# Deploy token gating contract
TGATING_CONTRACT_ID=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/token_gating.wasm \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK)

echo "Token Gating Contract: $TGATING_CONTRACT_ID"

# Save to environment file
echo "TOKEN_GATING_CONTRACT_ID=$TGATING_CONTRACT_ID" >> .env
```

### 3. Deploy Cross-Chain Bridge Contract

```bash
# Deploy cross-chain bridge contract
BRIDGE_CONTRACT_ID=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/cross_chain_bridge.wasm \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK)

echo "Bridge Contract: $BRIDGE_CONTRACT_ID"
echo "BRIDGE_CONTRACT_ID=$BRIDGE_CONTRACT_ID" >> .env
```

### 4. Initialize Contracts

#### Initialize Token Gating

```bash
export OWNER_ADDRESS="<your_owner_stellar_address>"

soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  initialize \
  --owner $OWNER_ADDRESS
```

#### Initialize Bridge

```bash
soroban contract invoke \
  --id $BRIDGE_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  initialize_bridge \
  --owner $OWNER_ADDRESS \
  --signature_threshold 2 \
  --cache_ttl_ledgers 300 \
  --eth_chain_id 1  # Ethereum mainnet (5 for Goerli)
```

## Phase 2: Configure Bridge Relays

### 1. Register Bridge Relays

```bash
# For each relay, register its Stellar address and ECDSA public key

# Relay 1
RELAY_1_ADDRESS="GXXXXX..."
RELAY_1_PUBKEY="0x04..."  # Uncompressed ECDSA public key (65 bytes)

soroban contract invoke \
  --id $BRIDGE_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  register_relay \
  --relay_address $RELAY_1_ADDRESS \
  --pubkey $RELAY_1_PUBKEY

# Repeat for Relay 2, Relay 3, etc.
```

### 2. Start Bridge Relays

Each relay should run the off-chain bridge service:

```bash
# Example relay configuration
cat > bridge-relay-config.json <<EOF
{
  "stellar": {
    "networkPassphrase": "Test SDF Network ; September 2015",
    "rpcUrl": "https://soroban-testnet.stellar.org",
    "contractId": "$BRIDGE_CONTRACT_ID",
    "relaySecret": "<relay_secret_key>"
  },
  "ethereum": {
    "chainId": 5,
    "rpcUrl": "https://goerli.infura.io/v3/<project_id>",
    "tokens": [
      {
        "address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
        "standard": "erc20",
        "decimals": 6
      },
      {
        "address": "0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d",
        "standard": "erc721"
      }
    ]
  },
  "verification": {
    "pollIntervalMs": 15000,
    "maxBlockAge": 256,
    "batchSize": 10
  }
}
EOF

# Start relay
node bridge-relay/index.ts --config bridge-relay-config.json
```

## Phase 3: Create Token Tiers

### 1. Free Tier

```bash
soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  create_token_tier \
  --tier_id free \
  --description "Basic public access" \
  --token_requirements '[]' \
  --purchase_price 0 \
  --duration_ledgers 0 \
  --tradeable false
```

### 2. Premium Tier (ERC-20)

```bash
# Premium tier requires 100 USDC (ERC-20)
soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  create_token_tier \
  --tier_id premium \
  --description "Premium analytics & real-time streams" \
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

### 3. Enterprise Tier (Multi-Chain)

```bash
# Enterprise: Hold 1000 USDC OR 1000 Stellar USDC OR any BAYC NFT
soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  create_token_tier \
  --tier_id enterprise \
  --description "Full audit trail, webhooks, support" \
  --token_requirements '[
    {
      "standard": "erc20",
      "contract_address": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
      "token_id": 0,
      "required_amount": 1000000000
    },
    {
      "standard": "stellar",
      "contract_address": "GBUQWP3BOUZX34ULNQG23RQ6F4YUSXHTEHLLVCAO76UZA34M6LSCCPR",
      "token_id": 0,
      "required_amount": 1000000000
    },
    {
      "standard": "erc721",
      "contract_address": "0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d",
      "token_id": 0,
      "required_amount": 1
    }
  ]' \
  --purchase_price 10000000 \
  --duration_ledgers 157680000 \
  --tradeable true
```

### 4. Configure Stream Access Control

```bash
# Require "premium" tier for premium-analytics stream
soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  set_stream_access_control \
  --event_type premium-analytics \
  --required_tier premium \
  --premium true

# Require "enterprise" tier for real-time stream
soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  set_stream_access_control \
  --event_type real-time-events \
  --required_tier enterprise \
  --premium true
```

## Phase 4: Deploy REST API

### 1. Configure API Service

```bash
cd api/rest

# Copy environment template
cp .env.example .env

# Configure environment
cat >> .env <<EOF
CONTRACT_ID=$TGATING_CONTRACT_ID
BRIDGE_CONTRACT_ID=$BRIDGE_CONTRACT_ID
RPC_URL=$RPC_URL
NETWORK=$NETWORK
API_PORT=3000
LOG_LEVEL=info
CACHE_TTL_SECONDS=300
RATE_LIMIT_MAX_REQUESTS=100
RATE_LIMIT_WINDOW_MS=60000
EOF
```

### 2. Build and Start API Service

```bash
# Install dependencies
npm install

# Build TypeScript
npm run build

# Start service
npm start

# Or with Docker
docker build -t audit-ledger-api:token-gating -f Dockerfile .
docker run -p 3000:3000 --env-file .env audit-ledger-api:token-gating
```

### 3. Test API Endpoints

```bash
# List tiers
curl http://localhost:3000/token-gating/tiers

# Check user access
curl http://localhost:3000/token-gating/users/GXXXXXX/tiers

# Verify balance
curl -X POST http://localhost:3000/token-gating/verify-balance \
  -H "Content-Type: application/json" \
  -d '{
    "user_address": "GXXXXXX",
    "token_standard": "erc20",
    "contract_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    "required_amount": 1000000000
  }'

# Health check
curl http://localhost:3000/token-gating/health/token-gating
```

## Phase 5: Deploy GraphQL API

### 1. Configure GraphQL Service

```bash
cd api/graphql

cp .env.example .env
cat >> .env <<EOF
CONTRACT_ID=$TGATING_CONTRACT_ID
BRIDGE_CONTRACT_ID=$BRIDGE_CONTRACT_ID
RPC_URL=$RPC_URL
GRAPHQL_PORT=4000
EOF
```

### 2. Build and Start

```bash
npm install
npm run build
npm start
```

### 3. Test GraphQL Queries

```bash
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{
    "query": "{ tokenTiers { tierId description purchasePrice } }"
  }'
```

## Phase 6: Deploy WebSocket Server

### 1. Configure WebSocket

```bash
cd api/ws

cp .env.example .env
echo "WS_PORT=5000" >> .env
```

### 2. Start WebSocket Server

```bash
npm install
npm start

# Or with Docker
docker build -t audit-ledger-ws:token-gating -f Dockerfile .
docker run -p 5000:5000 --env-file .env audit-ledger-ws:token-gating
```

### 3. Test WebSocket Connection

```bash
# Using wscat
npm install -g wscat
wscat -c ws://localhost:5000/token-gating

# Subscribe to marketplace events
{"action":"subscribe","channel":"marketplace:*"}

# Subscribe to tier updates
{"action":"subscribe","channel":"tiers:premium"}

# Unsubscribe
{"action":"unsubscribe","channel":"marketplace:*"}
```

## Phase 7: Docker Compose Orchestration

### 1. Create docker-compose.yml

```yaml
version: '3.8'

services:
  rest-api:
    build:
      context: ./api/rest
      dockerfile: Dockerfile
    ports:
      - "3000:3000"
    env_file: .env
    environment:
      - SERVICE=rest-api
      - LOG_LEVEL=info
    depends_on:
      - soroban-rpc

  graphql:
    build:
      context: ./api/graphql
      dockerfile: Dockerfile
    ports:
      - "4000:4000"
    env_file: .env
    environment:
      - SERVICE=graphql
    depends_on:
      - soroban-rpc

  websocket:
    build:
      context: ./api/ws
      dockerfile: Dockerfile
    ports:
      - "5000:5000"
    env_file: .env
    environment:
      - SERVICE=websocket

  relay-1:
    build:
      context: ./bridge/relayer
      dockerfile: Dockerfile
    env_file: .env.relay1
    environment:
      - RELAY_NAME=relay-1
    depends_on:
      - soroban-rpc

  relay-2:
    build:
      context: ./bridge/relayer
      dockerfile: Dockerfile
    env_file: .env.relay2
    environment:
      - RELAY_NAME=relay-2
    depends_on:
      - soroban-rpc

  soroban-rpc:
    image: stellar/soroban-rpc:21.2.0
    ports:
      - "8000:8000"
    environment:
      - NETWORK_PASSPHRASE=Test SDF Network ; September 2015
      - RPC_MODE=horizon
```

### 2. Start Full Stack

```bash
docker-compose up --build

# Or in background
docker-compose up -d --build

# View logs
docker-compose logs -f

# Stop
docker-compose down
```

## Monitoring & Operations

### 1. Monitor Bridge Relays

```bash
# Check relay health
curl http://relay-1:8080/health

# Check attestation latency
curl http://relay-1:8080/metrics | grep relay_attestation_latency_ms

# Check failed verifications
curl http://relay-1:8080/metrics | grep verification_failures_total
```

### 2. Monitor Contract State

```bash
# Check tier count
soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  total_tiers

# Check marketplace listings
curl http://localhost:3000/token-gating/marketplace/listings
```

### 3. Alerts & Dashboards

Set up Prometheus + Grafana to monitor:
- Bridge relay attestation latency
- Failed verifications per relay
- Marketplace transaction volume
- Contract state growth
- API response times

## Testing the Full Flow

### Step 1: User Acquires Token

```bash
# User obtains 100 USDC on Ethereum testnet
# OR 100 Stellar USDC
# OR holds a BAYC NFT
```

### Step 2: Trigger Bridge Verification

```bash
# REST API
curl -X POST http://localhost:3000/token-gating/verify-balance \
  -H "Authorization: Bearer <user_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "user_address": "GUSER...",
    "token_standard": "erc20",
    "contract_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    "required_amount": 100000000
  }'
```

### Step 3: Subscribe to Verification Event

```bash
# WebSocket
{"action":"subscribe","channel":"verification:GUSER..."}

# Wait for VERIFICATION_COMPLETED message
```

### Step 4: Check Stream Access

```bash
curl http://localhost:3000/token-gating/streams/premium-analytics/access?user_address=GUSER...

# Response: {"has_access": true, "required_tier": "premium"}
```

### Step 5: Purchase from Marketplace

```bash
# GraphQL
mutation {
  purchaseFromMarketplace(listingId: 101, quantity: 1) {
    success
    tierId
    expiresAt
  }
}
```

### Step 6: Subscribe to Purchase Event

```bash
# WebSocket
{"action":"subscribe","channel":"marketplace:premium"}

# Receives PURCHASE_COMPLETED message
```

## Troubleshooting

### Bridge Relay Not Submitting Attestations

1. Check relay logs: `docker logs <relay-container>`
2. Verify relay is registered: `soroban contract invoke ... get_relays`
3. Check Ethereum RPC connectivity
4. Verify ECDSA key format (65 bytes, uncompressed)

### Verification Cache Expired

1. Configure longer TTL: `cache_ttl_ledgers: 600` (vs. default 300)
2. Or manually re-trigger verification
3. Check bridge relay availability

### Marketplace Listing Not Visible

1. Verify listing is active (not cancelled)
2. Check quantity > 0 (or = 0 for unlimited)
3. Confirm tier is enabled
4. Clear API cache: `redis-cli FLUSHALL`

### High API Latency

1. Check contract state size
2. Implement pagination for listings
3. Archive old listings to off-chain storage
4. Scale API horizontally behind load balancer

## Rollback Procedure

```bash
# Disable tier without rolling back contract
soroban contract invoke \
  --id $TGATING_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  set_tier_enabled \
  --tier_id premium \
  --enabled false

# Deactivate relay
soroban contract invoke \
  --id $BRIDGE_CONTRACT_ID \
  --source $SOROBAN_SECRET_KEY \
  --network $NETWORK \
  -- \
  deactivate_relay \
  --relay_address $RELAY_ADDRESS
```

## Next Steps

1. ✅ Deploy contracts
2. ✅ Configure relays
3. ✅ Create tiers
4. ✅ Deploy APIs
5. ⏳ Load testing with synthetic marketplace activity
6. ⏳ Security audit of bridge relay implementation
7. ⏳ Production deployment checklist
8. ⏳ User education & documentation

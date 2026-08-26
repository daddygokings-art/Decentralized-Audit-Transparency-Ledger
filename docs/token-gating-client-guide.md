# Token Gating Client Guide

Usage examples for integrating token-gated access into applications.

## REST API Client Examples

### JavaScript/TypeScript

```typescript
import fetch from 'node-fetch';

class TokenGatingClient {
  private apiUrl: string;
  private token: string;

  constructor(apiUrl: string, authToken: string) {
    this.apiUrl = apiUrl;
    this.token = authToken;
  }

  private async request(
    method: string,
    endpoint: string,
    body?: Record<string, any>
  ): Promise<any> {
    const response = await fetch(`${this.apiUrl}${endpoint}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.token}`,
      },
      body: body ? JSON.stringify(body) : undefined,
    });

    if (!response.ok) {
      throw new Error(`API error: ${response.status}`);
    }

    return response.json();
  }

  // ======================================================================
  // Tier Queries
  // ======================================================================

  async listTiers(): Promise<Tier[]> {
    const result = await this.request('GET', '/token-gating/tiers');
    return result.data;
  }

  async getTier(tierId: string): Promise<Tier> {
    const result = await this.request('GET', `/token-gating/tiers/${tierId}`);
    return result.data;
  }

  async getUserTiers(userAddress: string): Promise<TierHolding[]> {
    const result = await this.request(
      'GET',
      `/token-gating/users/${userAddress}/tiers`
    );
    return result.tiers;
  }

  // ======================================================================
  // Access Verification
  // ======================================================================

  async checkStreamAccess(
    userAddress: string,
    eventType: string
  ): Promise<StreamAccessResponse> {
    const result = await this.request(
      'GET',
      `/token-gating/streams/${eventType}/access?user_address=${userAddress}`
    );
    return result.data;
  }

  async verifyTokenBalance(request: {
    user_address: string;
    token_standard: 'stellar' | 'erc20' | 'erc721' | 'erc1155';
    contract_address: string;
    token_id?: number;
    required_amount: number;
  }): Promise<VerificationResponse> {
    const result = await this.request('POST', '/token-gating/verify-balance', request);
    return result.data;
  }

  async verifyAndGrantAccess(
    userAddress: string,
    tierId: string,
    tokenStandard: string,
    contractAddress: string,
    requiredAmount: number
  ): Promise<TierHolding> {
    const result = await this.request(
      'POST',
      '/token-gating/verify-and-grant',
      {
        user_address: userAddress,
        tier_id: tierId,
        token_standard: tokenStandard,
        contract_address: contractAddress,
        required_amount: requiredAmount,
      }
    );
    return result.data;
  }

  // ======================================================================
  // Marketplace Operations
  // ======================================================================

  async listMarketplaceListings(filters?: {
    tierId?: string;
    sortBy?: 'price' | 'newest';
    limit?: number;
  }): Promise<MarketplaceListing[]> {
    const params = new URLSearchParams();
    if (filters) {
      if (filters.tierId) params.append('tier_id', filters.tierId);
      if (filters.sortBy) params.append('sort_by', filters.sortBy);
      if (filters.limit) params.append('limit', filters.limit.toString());
    }

    const result = await this.request(
      'GET',
      `/token-gating/marketplace/listings?${params.toString()}`
    );
    return result.data;
  }

  async createListing(request: {
    tier_id: string;
    price: number;
    quantity?: number;
  }): Promise<MarketplaceListing> {
    const result = await this.request(
      'POST',
      '/token-gating/marketplace/list',
      request
    );
    return result.data;
  }

  async purchaseFromMarketplace(request: {
    listing_id: number;
    quantity?: number;
  }): Promise<PurchaseResponse> {
    const result = await this.request(
      'POST',
      '/token-gating/marketplace/purchase',
      request
    );
    return result.data;
  }

  async cancelListing(listingId: number): Promise<void> {
    await this.request('DELETE', `/token-gating/marketplace/listings/${listingId}`);
  }

  // ======================================================================
  // Health Check
  // ======================================================================

  async checkHealth(): Promise<HealthStatus> {
    const result = await this.request('GET', '/token-gating/health/token-gating');
    return result;
  }
}

// ============================================================================
// Usage Examples
// ============================================================================

const client = new TokenGatingClient(
  'http://localhost:3000',
  'your-auth-token'
);

// List all tiers
const tiers = await client.listTiers();
console.log('Available tiers:', tiers);

// Check user's current access
const userTiers = await client.getUserTiers('GUSER...');
console.log('Your tiers:', userTiers);

// Check access to premium stream
const access = await client.checkStreamAccess('GUSER...', 'premium-analytics');
if (access.has_access) {
  console.log('You have access to premium analytics');
} else {
  console.log(`You need "${access.required_tier}" tier for access`);
}

// Verify ERC-20 balance and grant access
const verification = await client.verifyTokenBalance({
  user_address: '0x...',  // Ethereum address
  token_standard: 'erc20',
  contract_address: '0xdac17f958d2ee523a2206206994597c13d831ec7',
  required_amount: 1000000000, // 1000 USDC with 6 decimals
});

if (verification.verified) {
  const holding = await client.verifyAndGrantAccess(
    'GUSER...',
    'premium',
    'erc20',
    '0xdac17f958d2ee523a2206206994597c13d831ec7',
    1000000000
  );
  console.log('Tier granted:', holding);
}

// Browse marketplace
const listings = await client.listMarketplaceListings({
  tierId: 'premium',
  sortBy: 'price',
  limit: 10,
});

console.log('Available premium tiers for sale:', listings);

// Purchase from marketplace
const purchase = await client.purchaseFromMarketplace({
  listing_id: listings[0].listing_id,
});

console.log('Purchase complete:', purchase);

// List your tier for resale
const myListing = await client.createListing({
  tier_id: 'premium',
  price: 500_000,  // 0.05 XLM in stroops
  quantity: 0,      // Unlimited quantity
});

console.log('Listing created:', myListing);
```

### Python

```python
import requests
from typing import Dict, List, Optional

class TokenGatingClient:
    def __init__(self, api_url: str, auth_token: str):
        self.api_url = api_url
        self.auth_token = auth_token
        self.headers = {
            'Content-Type': 'application/json',
            'Authorization': f'Bearer {auth_token}',
        }

    def request(self, method: str, endpoint: str, json: Optional[Dict] = None) -> Dict:
        url = f"{self.api_url}{endpoint}"
        response = requests.request(method, url, headers=self.headers, json=json)
        response.raise_for_status()
        return response.json()

    def list_tiers(self) -> List[Dict]:
        result = self.request('GET', '/token-gating/tiers')
        return result['data']

    def get_user_tiers(self, user_address: str) -> List[Dict]:
        result = self.request('GET', f'/token-gating/users/{user_address}/tiers')
        return result['tiers']

    def check_stream_access(self, user_address: str, event_type: str) -> Dict:
        result = self.request(
            'GET',
            f'/token-gating/streams/{event_type}/access?user_address={user_address}'
        )
        return result['data']

    def verify_token_balance(
        self,
        user_address: str,
        token_standard: str,
        contract_address: str,
        required_amount: int,
        token_id: Optional[int] = None,
    ) -> Dict:
        payload = {
            'user_address': user_address,
            'token_standard': token_standard,
            'contract_address': contract_address,
            'required_amount': required_amount,
        }
        if token_id:
            payload['token_id'] = token_id

        result = self.request('POST', '/token-gating/verify-balance', json=payload)
        return result['data']

    def list_marketplace_listings(
        self,
        tier_id: Optional[str] = None,
        sort_by: str = 'price',
        limit: int = 20,
    ) -> List[Dict]:
        params = {
            'sort_by': sort_by,
            'limit': str(limit),
        }
        if tier_id:
            params['tier_id'] = tier_id

        query = '&'.join(f'{k}={v}' for k, v in params.items())
        result = self.request('GET', f'/token-gating/marketplace/listings?{query}')
        return result['data']

    def purchase_from_marketplace(self, listing_id: int, quantity: int = 1) -> Dict:
        result = self.request(
            'POST',
            '/token-gating/marketplace/purchase',
            json={'listing_id': listing_id, 'quantity': quantity}
        )
        return result['data']

# Usage
client = TokenGatingClient('http://localhost:3000', 'your-auth-token')

# Get tiers
tiers = client.list_tiers()
for tier in tiers:
    print(f"{tier['tier_id']}: {tier['description']} - {tier['purchase_price']} stroops")

# Check access
access = client.check_stream_access('GUSER...', 'premium-analytics')
print(f"Has access: {access['has_access']}")

# Verify balance
verification = client.verify_token_balance(
    user_address='GUSER...',
    token_standard='erc20',
    contract_address='0xdac17f958d2ee523a2206206994597c13d831ec7',
    required_amount=1_000_000_000,
)
print(f"Verified: {verification['verified']}, Balance: {verification['balance']}")

# Buy from marketplace
listings = client.list_marketplace_listings(tier_id='premium')
if listings:
    purchase = client.purchase_from_marketplace(listings[0]['listing_id'])
    print(f"Purchase successful: {purchase['success']}")
```

## GraphQL Client Examples

### Apollo Client (React)

```typescript
import { ApolloClient, InMemoryCache, HttpLink, gql } from '@apollo/client';

const client = new ApolloClient({
  link: new HttpLink({
    uri: 'http://localhost:4000/graphql',
    credentials: 'include',
  }),
  cache: new InMemoryCache(),
});

// List all tiers
const TIERS_QUERY = gql`
  query {
    tokenTiers {
      tierId
      description
      purchasePrice
      durationLedgers
      activeHolderCount
      tradeable
    }
  }
`;

const { data, loading, error } = await client.query({ query: TIERS_QUERY });

// Check user's tier holdings
const USER_TIERS_QUERY = gql`
  query UserTiers($userAddress: String!) {
    userTierHoldings(userAddress: $userAddress) {
      tierId
      expiryLedger
      verified
      timeRemaining
    }
  }
`;

const userTiers = await client.query({
  query: USER_TIERS_QUERY,
  variables: { userAddress: 'GUSER...' },
});

// Check stream access
const STREAM_ACCESS_QUERY = gql`
  query StreamAccess($userAddress: String!, $eventType: String!) {
    userStreamAccess(userAddress: $userAddress, eventType: $eventType) {
      hasAccess
      requiredTier
      currentTier
      expiresAt
    }
  }
`;

// Subscribe to marketplace updates
const LISTING_SUBSCRIPTION = gql`
  subscription ListingUpdated($tierId: String) {
    listingUpdated(tierId: $tierId) {
      listingId
      tierId
      price
      quantity
      seller
      active
    }
  }
`;

// Purchase from marketplace
const PURCHASE_MUTATION = gql`
  mutation Purchase($listingId: Int!, $quantity: Int) {
    purchaseFromMarketplace(listingId: $listingId, quantity: $quantity) {
      success
      tierId
      expiresAt
      transactionHash
    }
  }
`;

const purchase = await client.mutate({
  mutation: PURCHASE_MUTATION,
  variables: { listingId: 101 },
});
```

## WebSocket Real-Time Updates

### Browser JavaScript

```javascript
class TokenGatingWsClient {
  constructor(wsUrl) {
    this.ws = new WebSocket(wsUrl);
    this.listeners = new Map();
    this.messageId = 0;

    this.ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      console.log('Received:', message);

      // Emit to listeners
      const listeners = this.listeners.get(message.type) || [];
      listeners.forEach(callback => callback(message.payload));
    };

    this.ws.onopen = () => {
      console.log('Connected to token gating WebSocket');
    };

    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
  }

  subscribe(channel, filters = {}) {
    this.ws.send(JSON.stringify({
      action: 'subscribe',
      channel,
      filters,
    }));
  }

  unsubscribe(channel) {
    this.ws.send(JSON.stringify({
      action: 'unsubscribe',
      channel,
    }));
  }

  on(messageType, callback) {
    if (!this.listeners.has(messageType)) {
      this.listeners.set(messageType, []);
    }
    this.listeners.get(messageType).push(callback);
  }

  off(messageType, callback) {
    const listeners = this.listeners.get(messageType) || [];
    const index = listeners.indexOf(callback);
    if (index > -1) {
      listeners.splice(index, 1);
    }
  }

  close() {
    this.ws.close();
  }
}

// Usage
const wsClient = new TokenGatingWsClient('ws://localhost:5000/token-gating');

// Listen for marketplace purchases
wsClient.on('PURCHASE_COMPLETED', (purchase) => {
  console.log('New purchase:', purchase);
  document.querySelector('#latest-purchase').textContent =
    `${purchase.tier_id} sold for ${purchase.price} stroops`;
});

// Subscribe to premium tier marketplace
wsClient.subscribe('marketplace:premium', { active: true });

// Listen for tier grants (for current user)
wsClient.on('TIER_GRANTED', (holding) => {
  console.log('Tier granted:', holding);
  location.reload(); // Refresh to show new access
});

wsClient.subscribe('tiers:GUSER...');

// Listen for verification updates
wsClient.on('VERIFICATION_COMPLETED', (verification) => {
  console.log('Verification completed:', verification);
  if (verification.verified_balance > 0) {
    // Grant user access
  }
});

wsClient.subscribe('verification:GUSER...');
```

## Integration with Audit Ledger

### Restrict Premium Stream Access

```typescript
import { AuditLedgerClient } from '@audit-ledger/sdk';
import { TokenGatingClient } from '@audit-ledger/token-gating';

const auditClient = new AuditLedgerClient(contractId, rpcUrl);
const gatingClient = new TokenGatingClient(apiUrl, authToken);

async function logPremiumEvent(
  submitter: string,
  eventType: string,
  metadata: Buffer,
  userAddress: string
) {
  // Check token gating access
  const access = await gatingClient.checkStreamAccess(userAddress, eventType);
  
  if (!access.has_access) {
    throw new Error(
      `User needs "${access.required_tier}" tier to log this event type`
    );
  }

  // Log event to audit ledger
  const eventId = await auditClient.logEvent(
    submitter,
    eventType,
    metadata
  );

  return eventId;
}

// Usage
const eventId = await logPremiumEvent(
  'GSUBMITTER...',
  'premium-analytics',
  Buffer.from(JSON.stringify({ data: 'sensitive' })),
  'GUSER...'
);
```

### Export Premium Events

```typescript
async function exportPremiumAuditTrail(
  userAddress: string,
  eventType: string,
  startLedger: number,
  endLedger: number
) {
  // Verify access
  const access = await gatingClient.checkStreamAccess(userAddress, eventType);
  if (!access.has_access) {
    throw new Error('Access denied');
  }

  // Export events
  const events = await auditClient.queryEventsByType(
    eventType,
    startLedger,
    endLedger,
    { format: 'json' }
  );

  return events;
}
```

## Error Handling

```typescript
interface ApiError {
  error: string;
  code?: number;
  details?: string;
}

async function handleTokenGatingError(error: ApiError) {
  switch (error.code) {
    case 403:
      // Insufficient tier
      console.error('Access denied:', error.error);
      // Prompt user to purchase tier
      break;
    case 408:
      // Verification timeout (bridge unavailable)
      console.error('Verification failed:', error.error);
      // Retry or use admin grant
      break;
    case 429:
      // Rate limit exceeded
      console.error('Rate limited:', error.error);
      // Backoff and retry
      break;
    default:
      console.error('Unknown error:', error);
  }
}
```

## Best Practices

1. **Cache user tiers** — Reduce API calls by caching for 5-10 minutes
2. **Preload marketplace** — Show listings before user initiates purchase
3. **Real-time notifications** — Use WebSocket for instant UI updates
4. **Graceful degradation** — If token gating unavailable, allow read-only access
5. **Verify server-side** — Always verify tier access server-side before granting
6. **Handle expiry** — Refresh access status before stream processing
7. **Batch verifications** — Combine multiple token checks in single API call
8. **Monitor latency** — Track verification latency to detect bridge issues

## Testing

```typescript
import { describe, it, expect, beforeEach } from '@jest/globals';

describe('Token Gating Client', () => {
  let client: TokenGatingClient;

  beforeEach(() => {
    client = new TokenGatingClient(
      'http://localhost:3000',
      'test-token'
    );
  });

  it('should list available tiers', async () => {
    const tiers = await client.listTiers();
    expect(tiers).toBeInstanceOf(Array);
    expect(tiers.length).toBeGreaterThan(0);
    expect(tiers[0]).toHaveProperty('tierId');
  });

  it('should verify tier access', async () => {
    const access = await client.checkStreamAccess(
      'GTEST...',
      'premium-analytics'
    );
    expect(access).toHaveProperty('has_access');
    expect(access).toHaveProperty('required_tier');
  });

  it('should handle marketplace purchase', async () => {
    const listings = await client.listMarketplaceListings({
      tierId: 'premium',
      limit: 1,
    });

    if (listings.length > 0) {
      const purchase = await client.purchaseFromMarketplace({
        listing_id: listings[0].listing_id,
      });
      expect(purchase.success).toBe(true);
    }
  });
});
```

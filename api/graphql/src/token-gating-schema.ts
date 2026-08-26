/// GraphQL schema extensions for token gating
/// 
/// Adds types and queries for:
/// - Token tier queries and mutations
/// - User access verification
/// - Marketplace listings and purchases
/// - Stream access control

import { gql } from 'graphql-tag';

export const tokenGatingTypeDefs = gql`
  # ========================================================================
  # Enums
  # ========================================================================

  enum TokenStandard {
    """Stellar native asset (XLM or custom asset)"""
    STELLAR_ASSET
    """Ethereum ERC-20 fungible token"""
    ERC20
    """Ethereum ERC-721 non-fungible token (NFT)"""
    ERC721
    """Ethereum ERC-1155 multi-token standard"""
    ERC1155
  }

  # ========================================================================
  # Types
  # ========================================================================

  type TokenSpec {
    """Token standard (Stellar, ERC-20, ERC-721, ERC-1155)"""
    standard: TokenStandard!
    """Issuer address (Stellar) or contract address (EVM)"""
    contractAddress: String!
    """Token ID for ERC-1155; 0 for others"""
    tokenId: BigInt!
    """Required minimum amount (scaled by decimals)"""
    requiredAmount: BigInt!
  }

  type TokenTier {
    """Unique tier identifier"""
    tierId: String!
    """Human-readable description of tier benefits"""
    description: String!
    """Token requirements (any one grants access)"""
    tokenRequirements: [TokenSpec!]!
    """Purchase price in XLM stroops"""
    purchasePrice: BigInt!
    """Duration in ledgers; 0 = permanent"""
    durationLedgers: Int!
    """Whether tier can be traded on marketplace"""
    tradeable: Boolean!
    """Whether tier is currently enabled"""
    enabled: Boolean!
    """Total number of active holders"""
    activeHolderCount: Int!
    """Number of marketplace listings for this tier"""
    listingCount: Int!
  }

  type TierHolding {
    """User holding the tier"""
    holder: String!
    """Tier identifier"""
    tierId: String!
    """Ledger sequence when tier expires (0 = permanent)"""
    expiryLedger: Int!
    """Timestamp when tier was purchased"""
    purchasedAt: Int!
    """Whether tier is still verified and valid"""
    verified: Boolean!
    """Remaining time in seconds (null if permanent)"""
    timeRemaining: Int
  }

  type VerificationRecord {
    """User address being verified"""
    user: String!
    """Token specification being verified"""
    tokenSpec: TokenSpec!
    """Last verified balance"""
    verifiedBalance: BigInt!
    """Ledger height of last verification"""
    verifiedAtLedger: Int!
    """Verification cache TTL in ledgers"""
    ttlLedgers: Int!
    """Bridge relay that performed verification"""
    verifiedByBridge: String!
    """Whether verification is currently valid"""
    isValid: Boolean!
  }

  type StreamAccessControl {
    """Event type being gated"""
    eventType: String!
    """Minimum tier required for access"""
    requiredTier: String!
    """Whether this is a premium stream"""
    premium: Boolean!
    """Number of unique users with access"""
    accessedByCount: Int!
  }

  type UserStreamAccess {
    """Event type"""
    eventType: String!
    """Whether user has access"""
    hasAccess: Boolean!
    """Required tier"""
    requiredTier: String!
    """User's current tier (if any)"""
    currentTier: String
    """Expiration timestamp (null if permanent)"""
    expiresAt: Int
  }

  type MarketplaceListing {
    """Unique listing identifier"""
    listingId: Int!
    """Tier being sold"""
    tierId: String!
    """Seller address"""
    seller: String!
    """Price in XLM stroops"""
    price: BigInt!
    """Quantity available (0 = unlimited)"""
    quantity: Int!
    """Whether listing is active"""
    active: Boolean!
    """Timestamp when listing was created"""
    createdAt: Int!
    """Number of times purchased from this listing"""
    purchaseCount: Int!
    """Tier details"""
    tier: TokenTier!
  }

  type MarketplaceListingConnection {
    """List of listings"""
    edges: [MarketplaceListingEdge!]!
    """Pagination info"""
    pageInfo: PageInfo!
    """Total count of listings"""
    totalCount: Int!
  }

  type MarketplaceListingEdge {
    """The listing"""
    node: MarketplaceListing!
    """Cursor for pagination"""
    cursor: String!
  }

  type PageInfo {
    """Whether there are more results"""
    hasNextPage: Boolean!
    """Whether there are previous results"""
    hasPreviousPage: Boolean!
    """Cursor for next page"""
    nextCursor: String
    """Cursor for previous page"""
    prevCursor: String
  }

  type MarketplacePurchaseResult {
    """Whether purchase succeeded"""
    success: Boolean!
    """Listing ID"""
    listingId: Int!
    """Tier ID"""
    tierId: String!
    """Expiration timestamp (null if permanent)"""
    expiresAt: Int
    """Transaction hash"""
    transactionHash: String!
    """Timestamp of purchase"""
    purchasedAt: Int!
  }

  type TokenGatingStats {
    """Total number of active tiers"""
    totalTiers: Int!
    """Total number of unique users with tiers"""
    totalTierHolders: Int!
    """Total marketplace listings"""
    totalListings: Int!
    """Total completed purchases"""
    totalPurchases: Int!
    """Total trading volume in stroops"""
    totalTradingVolume: BigInt!
    """Aggregate statistics by tier"""
    tierStats: [TierStatistics!]!
  }

  type TierStatistics {
    """Tier identifier"""
    tierId: String!
    """Number of active holders"""
    holderCount: Int!
    """Average holding duration in days"""
    avgHoldingDays: Float!
    """Number of marketplace listings"""
    listingCount: Int!
    """Average listing price in stroops"""
    avgPrice: BigInt!
    """Total volume traded in stroops"""
    totalVolume: BigInt!
    """Newest listing price in stroops"""
    newestPrice: BigInt
  }

  # ========================================================================
  # Queries
  # ========================================================================

  extend type Query {
    """Get all available token tiers"""
    tokenTiers(
      """Filter by enabled status"""
      enabled: Boolean
      """Pagination limit"""
      limit: Int
      """Pagination offset"""
      offset: Int
    ): [TokenTier!]!

    """Get a specific tier by ID"""
    tokenTier(tierId: String!): TokenTier

    """Get user's current tier holdings"""
    userTierHoldings(
      userAddress: String!
      """Include expired holdings"""
      includeExpired: Boolean
    ): [TierHolding!]!

    """Check if user has access to a tier"""
    hasUserTierAccess(
      userAddress: String!
      tierId: String!
    ): Boolean!

    """Check user's access to a premium stream"""
    userStreamAccess(
      userAddress: String!
      eventType: String!
    ): UserStreamAccess!

    """Get all user's stream access"""
    userStreamAccessList(
      userAddress: String!
      """Filter by premium streams only"""
      premiumOnly: Boolean
    ): [UserStreamAccess!]!

    """Get stream access control configuration"""
    streamAccessControl(eventType: String!): StreamAccessControl

    """List marketplace listings"""
    marketplaceListings(
      """Filter by tier"""
      tierId: String
      """Filter by seller"""
      seller: String
      """Sort by: PRICE_ASC, PRICE_DESC, NEWEST, OLDEST"""
      sortBy: String
      """Pagination limit"""
      limit: Int
      """Pagination cursor"""
      after: String
    ): MarketplaceListingConnection!

    """Get a specific marketplace listing"""
    marketplaceListing(listingId: Int!): MarketplaceListing

    """Get all active listings for a tier"""
    tierListings(
      tierId: String!
      """Sort by PRICE_ASC, PRICE_DESC, NEWEST"""
      sortBy: String
      limit: Int
    ): [MarketplaceListing!]!

    """Get current user's listings (requires authentication)"""
    myMarketplaceListings(
      """Include inactive listings"""
      includeInactive: Boolean
    ): [MarketplaceListing!]!

    """Verify user token balance across chains"""
    verifyTokenBalance(
      userAddress: String!
      tokenStandard: TokenStandard!
      contractAddress: String!
      tokenId: BigInt
      requiredAmount: BigInt!
    ): VerificationRecord

    """Get aggregated token gating statistics"""
    tokenGatingStats: TokenGatingStats!

    """Get verification cache record"""
    verificationCache(
      userAddress: String!
      contractAddress: String!
      tokenId: BigInt
    ): VerificationRecord
  }

  # ========================================================================
  # Mutations
  # ========================================================================

  extend type Mutation {
    """Create a new token tier (owner-only)"""
    createTokenTier(
      tierId: String!
      description: String!
      tokenRequirements: [TokenSpecInput!]!
      purchasePrice: BigInt!
      durationLedgers: Int!
      tradeable: Boolean!
    ): TokenTier!

    """Set tier enabled/disabled status (owner-only)"""
    setTierEnabled(
      tierId: String!
      enabled: Boolean!
    ): TokenTier!

    """Grant tier to user (owner-only)"""
    grantTierToUser(
      userAddress: String!
      tierId: String!
      durationLedgers: Int
    ): TierHolding!

    """Set access control for an event stream (owner-only)"""
    setStreamAccessControl(
      eventType: String!
      requiredTier: String!
      premium: Boolean!
    ): StreamAccessControl!

    """Create a marketplace listing"""
    createMarketplaceListing(
      tierId: String!
      price: BigInt!
      quantity: Int
    ): MarketplaceListing!

    """Update a marketplace listing"""
    updateMarketplaceListing(
      listingId: Int!
      price: BigInt
      quantity: Int
      active: Boolean
    ): MarketplaceListing!

    """Cancel a marketplace listing"""
    cancelMarketplaceListing(listingId: Int!): Boolean!

    """Purchase tier from marketplace"""
    purchaseFromMarketplace(
      listingId: Int!
      quantity: Int
    ): MarketplacePurchaseResult!

    """Verify token balance and grant tier if qualified"""
    verifyAndGrantTier(
      userAddress: String!
      tierId: String!
      tokenStandard: TokenStandard!
      contractAddress: String!
      tokenId: BigInt
      requiredAmount: BigInt!
    ): TierHolding!
  }

  # ========================================================================
  # Subscriptions
  # ========================================================================

  extend type Subscription {
    """Subscribe to tier changes"""
    tierUpdated(tierId: String): TokenTier!

    """Subscribe to marketplace listing changes"""
    listingUpdated(tierId: String): MarketplaceListing!

    """Subscribe to marketplace purchases"""
    purchaseCompleted(tierId: String): MarketplacePurchaseResult!

    """Subscribe to user tier changes"""
    userTierChanged(userAddress: String!): TierHolding!

    """Subscribe to verification status changes"""
    verificationStatusChanged(
      userAddress: String!
      contractAddress: String!
    ): VerificationRecord!

    """Subscribe to stream access changes"""
    streamAccessChanged(
      userAddress: String!
      eventType: String!
    ): UserStreamAccess!
  }

  # ========================================================================
  # Input Types
  # ========================================================================

  input TokenSpecInput {
    """Token standard"""
    standard: TokenStandard!
    """Contract address"""
    contractAddress: String!
    """Token ID"""
    tokenId: BigInt
    """Required amount"""
    requiredAmount: BigInt!
  }

  input MarketplaceListingInput {
    """Tier to list"""
    tierId: String!
    """Price in stroops"""
    price: BigInt!
    """Quantity available"""
    quantity: Int
  }

  input VerificationInput {
    """User address"""
    userAddress: String!
    """Token standard"""
    tokenStandard: TokenStandard!
    """Contract address"""
    contractAddress: String!
    """Token ID"""
    tokenId: BigInt
    """Required amount"""
    requiredAmount: BigInt!
  }

  # ========================================================================
  # Scalars
  # ========================================================================

  scalar BigInt
`;

export default tokenGatingTypeDefs;

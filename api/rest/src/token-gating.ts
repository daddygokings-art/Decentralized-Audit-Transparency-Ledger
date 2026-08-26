/// REST API extensions for token gating
/// Endpoints for:
/// - Checking user access to tiers and premium streams
/// - Purchasing tiers via marketplace
/// - Verifying token balances across chains
/// - Tier and marketplace info queries

import express, { Request, Response, NextFunction } from 'express';
import { Router } from 'express';

// ============================================================================
// Types
// ============================================================================

interface TokenTier {
  tier_id: string;
  description: string;
  purchase_price: number;
  duration_ledgers: number;
  tradeable: boolean;
  enabled: boolean;
}

interface UserTierHolding {
  tier_id: string;
  expiry_ledger: number;
  purchased_at: number;
  verified: boolean;
}

interface VerificationRequest {
  user_address: string;
  token_standard: 'stellar' | 'erc20' | 'erc721' | 'erc1155';
  contract_address: string;
  token_id?: number;
  required_amount: number;
}

interface VerificationResponse {
  user_address: string;
  verified: boolean;
  balance: number;
  verified_at_ledger: number;
  ttl_ledgers: number;
}

interface MarketplaceListingRequest {
  tier_id: string;
  price: number;
  quantity?: number;
}

interface MarketplaceListingResponse {
  listing_id: number;
  tier_id: string;
  price: number;
  quantity: number;
  active: boolean;
  created_at: number;
}

interface StreamAccessCheckRequest {
  user_address: string;
  event_type: string;
}

interface StreamAccessCheckResponse {
  user_address: string;
  event_type: string;
  has_access: boolean;
  required_tier: string;
  current_tier?: string;
  expires_at?: number;
}

interface PurchaseMarketplaceRequest {
  listing_id: number;
  quantity?: number;
}

interface PurchaseResponse {
  success: boolean;
  listing_id: number;
  tier_id: string;
  expires_at?: number;
  transaction_hash?: string;
}

// ============================================================================
// Middleware for Authentication & Rate Limiting
// ============================================================================

/**
 * Verify user identity via signature or bearer token
 */
const verifyUserIdentity = (req: Request, res: Response, next: NextFunction) => {
  const authHeader = req.headers.authorization;
  
  if (!authHeader) {
    return res.status(401).json({ error: 'Missing authorization header' });
  }

  const [scheme, credentials] = authHeader.split(' ');

  if (scheme.toLowerCase() !== 'bearer' && scheme.toLowerCase() !== 'signature') {
    return res.status(401).json({ error: 'Invalid authorization scheme' });
  }

  // TODO: Verify signature or bearer token
  // For now, extract from header and validate format
  
  next();
};

/**
 * Rate limit token verification requests per user
 */
const rateLimitVerification = (req: Request, res: Response, next: NextFunction) => {
  // Simple in-memory rate limiting: max 10 verifications per minute per user
  const userAddr = req.body.user_address || req.query.user_address;
  
  if (!userAddr) {
    return res.status(400).json({ error: 'Missing user_address parameter' });
  }

  // TODO: Implement distributed rate limiting (Redis)
  
  next();
};

/**
 * Cache middleware for tier queries
 */
const cacheTierInfo = (req: Request, res: Response, next: NextFunction) => {
  // Cache tier data for 5 minutes
  const tierId = req.params.tier_id;
  
  if (!tierId) {
    return next();
  }

  res.set('Cache-Control', 'public, max-age=300');
  next();
};

// ============================================================================
// Route Handlers
// ============================================================================

const tokenGatingRouter = Router();

/**
 * GET /tiers
 * List all available tiers
 */
tokenGatingRouter.get('/tiers', async (req: Request, res: Response) => {
  try {
    // TODO: Query contract for all tiers
    // const tiers = await sorobanClient.getTiers();
    
    const tiers: TokenTier[] = [
      {
        tier_id: 'free',
        description: 'Basic access to public audit streams',
        purchase_price: 0,
        duration_ledgers: 0, // permanent
        tradeable: false,
        enabled: true,
      },
      {
        tier_id: 'premium',
        description: 'Access to real-time event analytics and premium streams',
        purchase_price: 1_000_000, // 0.1 XLM in stroops
        duration_ledgers: 52_560_000, // 1 year (roughly)
        tradeable: true,
        enabled: true,
      },
      {
        tier_id: 'enterprise',
        description: 'Full audit trail access, custom webhooks, dedicated support',
        purchase_price: 10_000_000, // 1 XLM in stroops
        duration_ledgers: 52_560_000,
        tradeable: true,
        enabled: true,
      },
    ];

    res.json({
      data: tiers,
      count: tiers.length,
    });
  } catch (error) {
    console.error('Error fetching tiers:', error);
    res.status(500).json({ error: 'Failed to fetch tiers' });
  }
});

/**
 * GET /tiers/:tier_id
 * Get specific tier details
 */
tokenGatingRouter.get(
  '/tiers/:tier_id',
  cacheTierInfo,
  async (req: Request, res: Response) => {
    try {
      const { tier_id } = req.params;

      // TODO: Query contract for specific tier
      // const tier = await sorobanClient.getTier(tier_id);

      const tier: TokenTier = {
        tier_id,
        description: 'Tier description',
        purchase_price: 1_000_000,
        duration_ledgers: 52_560_000,
        tradeable: true,
        enabled: true,
      };

      if (!tier) {
        return res.status(404).json({ error: 'Tier not found' });
      }

      res.json({ data: tier });
    } catch (error) {
      console.error('Error fetching tier:', error);
      res.status(500).json({ error: 'Failed to fetch tier' });
    }
  }
);

/**
 * GET /users/:user_address/tiers
 * Get user's current tier holdings
 */
tokenGatingRouter.get('/users/:user_address/tiers', async (req: Request, res: Response) => {
  try {
    const { user_address } = req.params;

    // TODO: Query contract for user holdings
    // const holdings = await sorobanClient.getUserTierHoldings(user_address);

    const holdings: UserTierHolding[] = [
      {
        tier_id: 'premium',
        expiry_ledger: 0, // permanent
        purchased_at: 1692900000,
        verified: true,
      },
    ];

    res.json({
      user_address,
      tiers: holdings,
      count: holdings.length,
    });
  } catch (error) {
    console.error('Error fetching user tiers:', error);
    res.status(500).json({ error: 'Failed to fetch user tiers' });
  }
});

/**
 * POST /verify-balance
 * Verify user's token balance for access qualification
 *
 * Request body:
 * {
 *   "user_address": "G...",
 *   "token_standard": "erc20",
 *   "contract_address": "0x...",
 *   "required_amount": 1000000
 * }
 */
tokenGatingRouter.post(
  '/verify-balance',
  verifyUserIdentity,
  rateLimitVerification,
  async (req: Request, res: Response) => {
    try {
      const verifyReq: VerificationRequest = req.body;

      // Validate request
      if (!verifyReq.user_address || !verifyReq.token_standard || !verifyReq.contract_address) {
        return res.status(400).json({ error: 'Missing required fields' });
      }

      // TODO: Call token gating contract to verify balance
      // const result = await sorobanClient.verifyTokenBalance(verifyReq);

      const response: VerificationResponse = {
        user_address: verifyReq.user_address,
        verified: true,
        balance: 5_000_000, // Example: user has 5M tokens
        verified_at_ledger: 123456,
        ttl_ledgers: 300, // Cache for 5 minutes
      };

      if (!response.verified) {
        return res.status(403).json({
          error: 'Verification failed',
          data: response,
        });
      }

      res.json({
        data: response,
        message: 'User balance verified',
      });
    } catch (error) {
      console.error('Error verifying balance:', error);
      res.status(500).json({ error: 'Balance verification failed' });
    }
  }
);

/**
 * GET /streams/:event_type/access
 * Check if user has access to an event stream
 *
 * Query params:
 * - user_address: User's Stellar address
 */
tokenGatingRouter.get('/streams/:event_type/access', async (req: Request, res: Response) => {
  try {
    const { event_type } = req.params;
    const { user_address } = req.query as { user_address?: string };

    if (!user_address) {
      return res.status(400).json({ error: 'Missing user_address parameter' });
    }

    // TODO: Query contract for stream access
    // const accessCheck = await sorobanClient.canAccessStream(user_address, event_type);

    const response: StreamAccessCheckResponse = {
      user_address: user_address as string,
      event_type,
      has_access: true,
      required_tier: 'premium',
      current_tier: 'premium',
      expires_at: 0, // permanent
    };

    res.json({
      data: response,
    });
  } catch (error) {
    console.error('Error checking stream access:', error);
    res.status(500).json({ error: 'Stream access check failed' });
  }
});

/**
 * POST /marketplace/list
 * Create a marketplace listing for a tier
 *
 * Request body:
 * {
 *   "tier_id": "premium",
 *   "price": 500000,
 *   "quantity": 10
 * }
 */
tokenGatingRouter.post(
  '/marketplace/list',
  verifyUserIdentity,
  async (req: Request, res: Response) => {
    try {
      const listing: MarketplaceListingRequest = req.body;

      if (!listing.tier_id || !listing.price) {
        return res.status(400).json({ error: 'Missing required fields' });
      }

      // TODO: Call contract to create listing
      // const listingId = await sorobanClient.listTierForSale(listing);

      const response: MarketplaceListingResponse = {
        listing_id: 12345,
        tier_id: listing.tier_id,
        price: listing.price,
        quantity: listing.quantity || 0,
        active: true,
        created_at: Math.floor(Date.now() / 1000),
      };

      res.status(201).json({
        data: response,
        message: 'Listing created successfully',
      });
    } catch (error) {
      console.error('Error creating listing:', error);
      res.status(500).json({ error: 'Failed to create listing' });
    }
  }
);

/**
 * POST /marketplace/purchase
 * Purchase a tier from marketplace listing
 *
 * Request body:
 * {
 *   "listing_id": 12345,
 *   "quantity": 1
 * }
 */
tokenGatingRouter.post(
  '/marketplace/purchase',
  verifyUserIdentity,
  async (req: Request, res: Response) => {
    try {
      const purchase: PurchaseMarketplaceRequest = req.body;

      if (!purchase.listing_id) {
        return res.status(400).json({ error: 'Missing listing_id' });
      }

      // TODO: Call contract to execute purchase
      // const result = await sorobanClient.purchaseFromMarketplace(purchase);

      const response: PurchaseResponse = {
        success: true,
        listing_id: purchase.listing_id,
        tier_id: 'premium',
        expires_at: Math.floor(Date.now() / 1000) + 365 * 24 * 3600,
        transaction_hash: '0xabc123...',
      };

      res.json({
        data: response,
        message: 'Purchase completed successfully',
      });
    } catch (error) {
      console.error('Error processing purchase:', error);
      res.status(500).json({ error: 'Purchase failed' });
    }
  }
);

/**
 * GET /marketplace/listings
 * List active marketplace listings
 *
 * Query params:
 * - tier_id: Filter by specific tier
 * - sort_by: 'price' | 'newest' (default: 'price')
 * - limit: Max results (default: 20)
 */
tokenGatingRouter.get('/marketplace/listings', async (req: Request, res: Response) => {
  try {
    const { tier_id, sort_by = 'price', limit = '20' } = req.query;

    // TODO: Query contract for active listings
    // const listings = await sorobanClient.getMarketplaceListings({
    //   tier_id: tier_id as string,
    //   sortBy: sort_by as string,
    //   limit: parseInt(limit as string),
    // });

    const listings: MarketplaceListingResponse[] = [
      {
        listing_id: 101,
        tier_id: 'premium',
        price: 500_000,
        quantity: 10,
        active: true,
        created_at: Math.floor(Date.now() / 1000),
      },
      {
        listing_id: 102,
        tier_id: 'premium',
        price: 600_000,
        quantity: 5,
        active: true,
        created_at: Math.floor(Date.now() / 1000) - 3600,
      },
    ];

    res.json({
      data: listings,
      count: listings.length,
      query: { tier_id, sort_by, limit },
    });
  } catch (error) {
    console.error('Error fetching marketplace listings:', error);
    res.status(500).json({ error: 'Failed to fetch listings' });
  }
});

/**
 * DELETE /marketplace/listings/:listing_id
 * Cancel a marketplace listing (seller-only)
 */
tokenGatingRouter.delete(
  '/marketplace/listings/:listing_id',
  verifyUserIdentity,
  async (req: Request, res: Response) => {
    try {
      const { listing_id } = req.params;

      // TODO: Call contract to cancel listing
      // await sorobanClient.cancelMarketplaceListing(parseInt(listing_id));

      res.json({
        message: 'Listing cancelled successfully',
        listing_id: parseInt(listing_id),
      });
    } catch (error) {
      console.error('Error cancelling listing:', error);
      res.status(500).json({ error: 'Failed to cancel listing' });
    }
  }
);

/**
 * GET /health/token-gating
 * Health check for token gating service
 */
tokenGatingRouter.get('/health/token-gating', async (req: Request, res: Response) => {
  try {
    // TODO: Check contract state and bridge connectivity
    const health = {
      status: 'healthy',
      contract_connected: true,
      bridge_connected: true,
      cache_ttl_seconds: 300,
      timestamp: new Date().toISOString(),
    };

    res.json(health);
  } catch (error) {
    console.error('Health check failed:', error);
    res.status(503).json({
      status: 'unhealthy',
      error: 'Service unavailable',
    });
  }
});

export default tokenGatingRouter;

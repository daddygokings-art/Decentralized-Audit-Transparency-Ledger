/// WebSocket support for real-time token gating events
/// 
/// Broadcasts:
/// - Marketplace listing changes (new, updated, cancelled)
/// - Tier purchase completions
/// - User tier grants
/// - Verification status changes
/// - Stream access changes

import WebSocket from 'ws';
import { EventEmitter } from 'events';

// ============================================================================
// Types
// ============================================================================

interface WsMessage {
  type: string;
  payload: any;
  timestamp: number;
}

interface SubscriptionRequest {
  action: 'subscribe' | 'unsubscribe';
  channel: string;
  filters?: Record<string, any>;
}

interface Subscriber {
  ws: WebSocket;
  channels: Set<string>;
  userId?: string;
  filters: Map<string, Record<string, any>>;
}

enum MessageType {
  // Marketplace events
  LISTING_CREATED = 'LISTING_CREATED',
  LISTING_UPDATED = 'LISTING_UPDATED',
  LISTING_CANCELLED = 'LISTING_CANCELLED',
  PURCHASE_COMPLETED = 'PURCHASE_COMPLETED',
  
  // Tier events
  TIER_GRANTED = 'TIER_GRANTED',
  TIER_UPDATED = 'TIER_UPDATED',
  TIER_EXPIRED = 'TIER_EXPIRED',
  
  // Verification events
  VERIFICATION_COMPLETED = 'VERIFICATION_COMPLETED',
  VERIFICATION_FAILED = 'VERIFICATION_FAILED',
  VERIFICATION_EXPIRED = 'VERIFICATION_EXPIRED',
  
  // Stream events
  STREAM_ACCESS_GRANTED = 'STREAM_ACCESS_GRANTED',
  STREAM_ACCESS_REVOKED = 'STREAM_ACCESS_REVOKED',
  
  // System
  CONNECTED = 'CONNECTED',
  SUBSCRIBED = 'SUBSCRIBED',
  UNSUBSCRIBED = 'UNSUBSCRIBED',
  ERROR = 'ERROR',
}

// ============================================================================
// Channel Manager
// ============================================================================

class TokenGatingChannelManager extends EventEmitter {
  private subscribers: Map<string, Subscriber[]> = new Map();
  private marketplaceUpdates = 'marketplace:*';
  private tierUpdates = 'tiers:*';
  private verificationUpdates = 'verification:*';
  private streamAccessUpdates = 'streams:*';

  /**
   * Register a subscriber to a channel
   */
  subscribe(ws: WebSocket, channel: string, userId?: string, filters?: Record<string, any>) {
    const subscriber: Subscriber = {
      ws,
      channels: new Set([channel]),
      userId,
      filters: new Map([[channel, filters || {}]]),
    };

    if (!this.subscribers.has(channel)) {
      this.subscribers.set(channel, []);
    }

    this.subscribers.get(channel)!.push(subscriber);

    const response: WsMessage = {
      type: MessageType.SUBSCRIBED,
      payload: {
        channel,
        message: `Subscribed to ${channel}`,
      },
      timestamp: Date.now(),
    };

    ws.send(JSON.stringify(response));
  }

  /**
   * Unsubscribe from a channel
   */
  unsubscribe(ws: WebSocket, channel: string) {
    const subscribers = this.subscribers.get(channel);
    if (!subscribers) return;

    const index = subscribers.findIndex(s => s.ws === ws);
    if (index !== -1) {
      subscribers.splice(index, 1);
    }

    const response: WsMessage = {
      type: MessageType.UNSUBSCRIBED,
      payload: {
        channel,
        message: `Unsubscribed from ${channel}`,
      },
      timestamp: Date.now(),
    };

    ws.send(JSON.stringify(response));
  }

  /**
   * Broadcast message to all subscribers of a channel
   */
  broadcast(channel: string, messageType: MessageType, payload: any) {
    const subscribers = this.subscribers.get(channel) || [];
    const message: WsMessage = {
      type: messageType,
      payload,
      timestamp: Date.now(),
    };

    for (const subscriber of subscribers) {
      // Check if subscriber matches filters
      const filters = subscriber.filters.get(channel) || {};
      if (this.matchesFilters(payload, filters)) {
        try {
          subscriber.ws.send(JSON.stringify(message));
        } catch (err) {
          console.error('Failed to send message to subscriber:', err);
        }
      }
    }
  }

  /**
   * Broadcast marketplace event
   */
  broadcastMarketplaceEvent(
    event: 'created' | 'updated' | 'cancelled' | 'purchased',
    listing: any
  ) {
    const messageType = {
      created: MessageType.LISTING_CREATED,
      updated: MessageType.LISTING_UPDATED,
      cancelled: MessageType.LISTING_CANCELLED,
      purchased: MessageType.PURCHASE_COMPLETED,
    }[event];

    // Broadcast to tier-specific channel and global marketplace channel
    this.broadcast(`marketplace:${listing.tier_id}`, messageType, listing);
    this.broadcast('marketplace:*', messageType, listing);
  }

  /**
   * Broadcast tier event
   */
  broadcastTierEvent(event: 'granted' | 'updated' | 'expired', holding: any) {
    const messageType = {
      granted: MessageType.TIER_GRANTED,
      updated: MessageType.TIER_UPDATED,
      expired: MessageType.TIER_EXPIRED,
    }[event];

    // Broadcast to user-specific and tier-specific channels
    if (holding.holder) {
      this.broadcast(`tiers:${holding.holder}`, messageType, holding);
    }
    this.broadcast(`tiers:${holding.tier_id}`, messageType, holding);
    this.broadcast('tiers:*', messageType, holding);
  }

  /**
   * Broadcast verification event
   */
  broadcastVerificationEvent(
    event: 'completed' | 'failed' | 'expired',
    verification: any
  ) {
    const messageType = {
      completed: MessageType.VERIFICATION_COMPLETED,
      failed: MessageType.VERIFICATION_FAILED,
      expired: MessageType.VERIFICATION_EXPIRED,
    }[event];

    // Broadcast to user-specific and token-specific channels
    this.broadcast(`verification:${verification.user}`, messageType, verification);
    this.broadcast(`verification:${verification.token_spec.contract_address}`, messageType, verification);
    this.broadcast('verification:*', messageType, verification);
  }

  /**
   * Broadcast stream access event
   */
  broadcastStreamAccessEvent(
    event: 'granted' | 'revoked',
    access: any
  ) {
    const messageType = {
      granted: MessageType.STREAM_ACCESS_GRANTED,
      revoked: MessageType.STREAM_ACCESS_REVOKED,
    }[event];

    // Broadcast to user-specific and stream-specific channels
    this.broadcast(`streams:${access.user_address}:${access.event_type}`, messageType, access);
    this.broadcast(`streams:${access.event_type}`, messageType, access);
    this.broadcast(`streams:${access.user_address}`, messageType, access);
    this.broadcast('streams:*', messageType, access);
  }

  /**
   * Clean up disconnected subscribers
   */
  removeSubscriber(ws: WebSocket) {
    for (const [channel, subscribers] of this.subscribers.entries()) {
      const index = subscribers.findIndex(s => s.ws === ws);
      if (index !== -1) {
        subscribers.splice(index, 1);
      }
    }
  }

  /**
   * Check if payload matches filters
   */
  private matchesFilters(payload: any, filters: Record<string, any>): boolean {
    for (const [key, value] of Object.entries(filters)) {
      if (Array.isArray(value)) {
        if (!value.includes(payload[key])) {
          return false;
        }
      } else if (payload[key] !== value) {
        return false;
      }
    }
    return true;
  }

  /**
   * Get subscriber count for a channel
   */
  getSubscriberCount(channel: string): number {
    return (this.subscribers.get(channel) || []).length;
  }

  /**
   * Get all active channels
   */
  getActiveChannels(): string[] {
    return Array.from(this.subscribers.keys()).filter(
      channel => (this.subscribers.get(channel) || []).length > 0
    );
  }
}

// ============================================================================
// WebSocket Server Setup
// ============================================================================

export function setupTokenGatingWebSocket(server: any): TokenGatingChannelManager {
  const manager = new TokenGatingChannelManager();
  const wss = new WebSocket.Server({ server, path: '/token-gating' });

  wss.on('connection', (ws: WebSocket) => {
    console.log('[TokenGating WS] New client connected');

    // Send welcome message
    const welcome: WsMessage = {
      type: MessageType.CONNECTED,
      payload: {
        message: 'Connected to token gating WebSocket',
        availableChannels: {
          marketplace: 'marketplace:* | marketplace:TIER_ID',
          tiers: 'tiers:* | tiers:USER_ADDRESS | tiers:TIER_ID',
          verification: 'verification:* | verification:USER_ADDRESS | verification:TOKEN_CONTRACT',
          streams: 'streams:* | streams:EVENT_TYPE | streams:USER_ADDRESS',
        },
      },
      timestamp: Date.now(),
    };

    ws.send(JSON.stringify(welcome));

    // Handle incoming messages
    ws.on('message', (data: string) => {
      try {
        const request: SubscriptionRequest = JSON.parse(data);

        if (request.action === 'subscribe') {
          manager.subscribe(ws, request.channel, undefined, request.filters);
        } else if (request.action === 'unsubscribe') {
          manager.unsubscribe(ws, request.channel);
        }
      } catch (err) {
        console.error('[TokenGating WS] Error parsing message:', err);

        const error: WsMessage = {
          type: MessageType.ERROR,
          payload: {
            error: 'Invalid message format',
            details: (err as Error).message,
          },
          timestamp: Date.now(),
        };

        ws.send(JSON.stringify(error));
      }
    });

    // Handle client disconnect
    ws.on('close', () => {
      console.log('[TokenGating WS] Client disconnected');
      manager.removeSubscriber(ws);
    });

    // Handle errors
    ws.on('error', (err: Error) => {
      console.error('[TokenGating WS] WebSocket error:', err);
    });
  });

  return manager;
}

// ============================================================================
// Example Usage & Integration
// ============================================================================

export class TokenGatingEventBroadcaster {
  constructor(private manager: TokenGatingChannelManager) {}

  /**
   * Called when a marketplace listing is created
   */
  onListingCreated(listing: any) {
    this.manager.broadcastMarketplaceEvent('created', {
      ...listing,
      created_at: Math.floor(Date.now() / 1000),
    });
  }

  /**
   * Called when a marketplace listing is updated
   */
  onListingUpdated(listing: any) {
    this.manager.broadcastMarketplaceEvent('updated', listing);
  }

  /**
   * Called when a marketplace listing is cancelled
   */
  onListingCancelled(listing: any) {
    this.manager.broadcastMarketplaceEvent('cancelled', listing);
  }

  /**
   * Called when a marketplace purchase is completed
   */
  onPurchaseCompleted(purchase: any) {
    this.manager.broadcastMarketplaceEvent('purchased', {
      ...purchase,
      timestamp: Math.floor(Date.now() / 1000),
    });
  }

  /**
   * Called when a tier is granted to a user
   */
  onTierGranted(holding: any) {
    this.manager.broadcastTierEvent('granted', holding);
  }

  /**
   * Called when a tier is updated
   */
  onTierUpdated(holding: any) {
    this.manager.broadcastTierEvent('updated', holding);
  }

  /**
   * Called when a tier expires
   */
  onTierExpired(holding: any) {
    this.manager.broadcastTierEvent('expired', holding);
  }

  /**
   * Called when token balance is verified
   */
  onVerificationCompleted(verification: any) {
    this.manager.broadcastVerificationEvent('completed', verification);
  }

  /**
   * Called when verification fails
   */
  onVerificationFailed(verification: any, error: string) {
    this.manager.broadcastVerificationEvent('failed', {
      ...verification,
      error,
    });
  }

  /**
   * Called when verification cache expires
   */
  onVerificationExpired(verification: any) {
    this.manager.broadcastVerificationEvent('expired', verification);
  }

  /**
   * Called when user gains stream access
   */
  onStreamAccessGranted(access: any) {
    this.manager.broadcastStreamAccessEvent('granted', access);
  }

  /**
   * Called when user loses stream access
   */
  onStreamAccessRevoked(access: any) {
    this.manager.broadcastStreamAccessEvent('revoked', access);
  }

  /**
   * Get channel statistics
   */
  getStatistics() {
    const channels = this.manager.getActiveChannels();
    return {
      totalChannels: channels.length,
      channels: channels.map(channel => ({
        name: channel,
        subscribers: this.manager.getSubscriberCount(channel),
      })),
      timestamp: Math.floor(Date.now() / 1000),
    };
  }
}

export default { setupTokenGatingWebSocket, TokenGatingEventBroadcaster };

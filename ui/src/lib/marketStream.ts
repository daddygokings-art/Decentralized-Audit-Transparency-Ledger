/**
 * Prediction Market Real-Time WebSocket Streaming Client
 *
 * Eliminates 3-5 second polling latency by pushing market updates,
 * price shifts, and resolution events in real-time over persistent WebSocket connections.
 * Saves bandwidth on static markets via delta encoding and selective subscription.
 */

export interface MarketStreamMessage {
  type: "market_update" | "orderbook_diff" | "resolution" | "heartbeat" | "subscribed" | "error";
  marketId?: string;
  data?: any;
  sequence?: number;
  timestamp?: number;
}

export type MarketEventHandler = (msg: MarketStreamMessage) => void;

export class MarketStreamClient {
  private url: string;
  private ws: WebSocket | null = null;
  private subscriptions: Set<string> = new Set();
  private handlers: Map<string, Set<MarketEventHandler>> = new Map();
  private reconnectAttempts = 0;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private isExplicitlyClosed = false;
  private maxReconnectDelay = 15000;

  constructor(url?: string) {
    this.url = url || (process.env.NEXT_PUBLIC_MARKET_WS_URL ?? "wss://api.auditledger.org/markets/stream");
  }

  public connect(): void {
    if (typeof window === "undefined") return;
    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
      return;
    }

    this.isExplicitlyClosed = false;

    try {
      this.ws = new WebSocket(this.url);

      this.ws.onopen = () => {
        this.reconnectAttempts = 0;
        // Resubscribe all active topics upon reconnection
        for (const topic of this.subscriptions) {
          this.send({ action: "subscribe", topic });
        }
      };

      this.ws.onmessage = (event) => {
        try {
          const payload: MarketStreamMessage = JSON.parse(event.data);
          this.dispatch(payload);
        } catch {
          // ignore non-json messages
        }
      };

      this.ws.onclose = () => {
        this.ws = null;
        if (!this.isExplicitlyClosed) {
          this.scheduleReconnect();
        }
      };

      this.ws.onerror = () => {
        // error handling handled via onclose
      };
    } catch {
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    const delay = Math.min(1000 * Math.pow(1.5, this.reconnectAttempts), this.maxReconnectDelay);
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => {
      this.connect();
    }, delay);
  }

  public subscribe(marketId: string, handler: MarketEventHandler): () => void {
    const topic = `market:${marketId}`;
    this.subscriptions.add(topic);

    if (!this.handlers.has(topic)) {
      this.handlers.set(topic, new Set());
    }
    this.handlers.get(topic)!.add(handler);

    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.send({ action: "subscribe", topic });
    } else {
      this.connect();
    }

    return () => {
      this.unsubscribe(marketId, handler);
    };
  }

  public unsubscribe(marketId: string, handler: MarketEventHandler): void {
    const topic = `market:${marketId}`;
    const topicHandlers = this.handlers.get(topic);
    if (topicHandlers) {
      topicHandlers.delete(handler);
      if (topicHandlers.size === 0) {
        this.handlers.delete(topic);
        this.subscriptions.delete(topic);
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
          this.send({ action: "unsubscribe", topic });
        }
      }
    }
  }

  private dispatch(msg: MarketStreamMessage): void {
    const topic = msg.marketId ? `market:${msg.marketId}` : null;
    if (topic && this.handlers.has(topic)) {
      for (const handler of this.handlers.get(topic)!) {
        handler(msg);
      }
    }

    // Global wildcard subscribers
    if (this.handlers.has("*")) {
      for (const handler of this.handlers.get("*")!) {
        handler(msg);
      }
    }
  }

  private send(payload: any): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(payload));
    }
  }

  public close(): void {
    this.isExplicitlyClosed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}

// Default singleton instance
export const marketStream = new MarketStreamClient();

"use client";

import { useEffect, useState, useCallback } from "react";
import { marketStream, MarketStreamMessage } from "@/lib/marketStream";
import type { MarketUpdateEvent } from "@/types";

export interface UseMarketStreamOptions {
  marketId: string;
  initialData?: Partial<MarketUpdateEvent>;
}

/**
 * React hook for consuming real-time prediction market updates via WebSockets.
 * Replaces expensive 3-5s polling with instant push deltas, eliminating idle bandwidth waste.
 */
export function useMarketStream({ marketId, initialData }: UseMarketStreamOptions) {
  const [data, setData] = useState<Partial<MarketUpdateEvent>>(initialData || {});
  const [isConnected, setIsConnected] = useState<boolean>(false);
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  const [latencyMs, setLatencyMs] = useState<number>(0);

  const handleMessage = useCallback((msg: MarketStreamMessage) => {
    if (msg.type === "market_update" || msg.type === "orderbook_diff" || msg.type === "resolution") {
      const now = Date.now();
      if (msg.timestamp) {
        setLatencyMs(Math.max(0, now - msg.timestamp));
      }
      setLastUpdated(now);
      if (msg.data) {
        setData((prev) => ({ ...prev, ...msg.data }));
      }
    }
  }, []);

  useEffect(() => {
    if (!marketId) return;

    setIsConnected(true);
    const unsubscribe = marketStream.subscribe(marketId, handleMessage);

    return () => {
      unsubscribe();
      setIsConnected(false);
    };
  }, [marketId, handleMessage]);

  return {
    data,
    isConnected,
    lastUpdated,
    latencyMs,
  };
}

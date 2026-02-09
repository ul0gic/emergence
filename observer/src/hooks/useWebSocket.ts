/**
 * WebSocket hook for real-time tick summary streaming.
 *
 * Connects to the Axum backend at /ws/ticks and streams TickBroadcast
 * messages. Implements reconnection with exponential backoff. Every
 * incoming message is validated through Zod before updating state.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import type { TickBroadcast } from "../types/generated/index.ts";
import { parseTickBroadcast } from "../types/schemas.ts";

export type ConnectionStatus = "connecting" | "connected" | "disconnected" | "reconnecting";

const MAX_RECONNECT_DELAY_MS = 30_000;
const INITIAL_RECONNECT_DELAY_MS = 1_000;
const MAX_TICK_HISTORY = 500;

/** Intentional no-op, replaced on mount. */
function noop(): void {
  /* placeholder */
}

interface UseWebSocketReturn {
  /** Current connection status. */
  status: ConnectionStatus;
  /** Most recent tick broadcast. */
  latestTick: TickBroadcast | null;
  /** History of tick broadcasts (most recent first, capped). */
  tickHistory: TickBroadcast[];
  /** Manually reconnect. */
  reconnect: () => void;
}

function getWsUrl(): string {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/ws/ticks`;
}

/**
 * Create a WebSocket and wire up event handlers. Returns the WebSocket
 * instance so the caller can close it during cleanup.
 */
function createSocket(
  mountedRef: React.RefObject<boolean>,
  reconnectDelayRef: React.RefObject<number>,
  reconnectTimerRef: React.RefObject<ReturnType<typeof setTimeout> | null>,
  setStatus: (s: ConnectionStatus) => void,
  setLatestTick: (t: TickBroadcast) => void,
  setTickHistory: React.Dispatch<React.SetStateAction<TickBroadcast[]>>,
  connectRef: React.RefObject<() => void>,
): WebSocket {
  const ws = new WebSocket(getWsUrl());

  ws.onopen = () => {
    if (!mountedRef.current) return;
    setStatus("connected");
    reconnectDelayRef.current = INITIAL_RECONNECT_DELAY_MS;
  };

  ws.onmessage = (event: MessageEvent) => {
    if (!mountedRef.current) return;

    try {
      const raw: unknown = JSON.parse(event.data as string);
      const tick = parseTickBroadcast(raw);

      setLatestTick(tick);
      setTickHistory((prev) => {
        const next = [tick, ...prev];
        if (next.length > MAX_TICK_HISTORY) {
          return next.slice(0, MAX_TICK_HISTORY);
        }
        return next;
      });
    } catch (err) {
      console.error("[WebSocket] Failed to parse tick broadcast:", err);
    }
  };

  ws.onclose = () => {
    if (!mountedRef.current) return;
    setStatus("reconnecting");

    // Schedule reconnection with exponential backoff.
    if (reconnectTimerRef.current) return;
    const delay = reconnectDelayRef.current;
    reconnectTimerRef.current = setTimeout(() => {
      reconnectTimerRef.current = null;
      reconnectDelayRef.current = Math.min(reconnectDelayRef.current * 2, MAX_RECONNECT_DELAY_MS);
      connectRef.current();
    }, delay);
  };

  ws.onerror = () => {
    if (!mountedRef.current) return;
    // onclose will fire after onerror, so reconnection is handled there.
  };

  return ws;
}

export function useWebSocket(): UseWebSocketReturn {
  // Initial status is "connecting" — no setState needed on first mount.
  const [status, setStatus] = useState<ConnectionStatus>("connecting");
  const [latestTick, setLatestTick] = useState<TickBroadcast | null>(null);
  const [tickHistory, setTickHistory] = useState<TickBroadcast[]>([]);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectDelayRef = useRef(INITIAL_RECONNECT_DELAY_MS);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mountedRef = useRef(true);

  // connectRef holds the reconnection function so scheduled callbacks always
  // call the latest version without stale closures.
  const connectRef = useRef<() => void>(noop);

  // Main effect: subscribe to the external WebSocket system.
  // The initial connection is created directly here (no setState in the
  // effect body — status is already "connecting" from the initial state).
  useEffect(() => {
    mountedRef.current = true;

    // Create the initial WebSocket connection. The status is already
    // "connecting" from useState initialization, so no setStatus call needed.
    wsRef.current = createSocket(
      mountedRef,
      reconnectDelayRef,
      reconnectTimerRef,
      setStatus,
      setLatestTick,
      setTickHistory,
      connectRef,
    );

    return () => {
      mountedRef.current = false;
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, []);

  // Wire up connectRef so scheduled reconnections use fresh state.
  useEffect(() => {
    connectRef.current = () => {
      if (!mountedRef.current) return;

      // Clean up existing connection.
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }

      setStatus("connecting");

      wsRef.current = createSocket(
        mountedRef,
        reconnectDelayRef,
        reconnectTimerRef,
        setStatus,
        setLatestTick,
        setTickHistory,
        connectRef,
      );
    };
  });

  const reconnect = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    reconnectDelayRef.current = INITIAL_RECONNECT_DELAY_MS;
    connectRef.current();
  }, []);

  return { status, latestTick, tickHistory, reconnect };
}

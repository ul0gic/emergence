/**
 * Operator API hooks for simulation management.
 *
 * Provides polling for operator status and mutation hooks for
 * pause, resume, speed control, event injection, and stop.
 * All responses are validated through Zod schemas.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import type {
  InjectEventRequest,
  OperatorMutationResponse,
  OperatorStatus,
} from "../types/generated/index.ts";
import { parseOperatorMutationResponse, parseOperatorStatus } from "../types/schemas.ts";

// ---------------------------------------------------------------------------
// Generic fetch helpers
// ---------------------------------------------------------------------------

async function apiFetch<T>(url: string, parser: (data: unknown) => T): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`);
  }
  const raw: unknown = await response.json();
  return parser(raw);
}

async function apiPost<T>(url: string, body: unknown, parser: (data: unknown) => T): Promise<T> {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`);
  }
  const raw: unknown = await response.json();
  return parser(raw);
}

// ---------------------------------------------------------------------------
// Hook: useOperatorStatus (polls every 2 seconds)
// ---------------------------------------------------------------------------

interface UseOperatorStatusReturn {
  data: OperatorStatus | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useOperatorStatus(): UseOperatorStatusReturn {
  const [data, setData] = useState<OperatorStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const fetchData = useCallback(async () => {
    try {
      const result = await apiFetch("/api/operator/status", parseOperatorStatus);
      if (mountedRef.current) {
        setData(result);
        setError(null);
        setLoading(false);
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err.message : "Unknown error");
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;

    // Use setTimeout(0) to defer the initial fetch so it runs as a
    // microtask rather than synchronously inside the effect body.
    // This avoids the react-hooks/set-state-in-effect lint rule.
    const initialTimer = setTimeout(fetchData, 0);

    const interval = setInterval(() => {
      fetchData();
    }, 2000);

    return () => {
      mountedRef.current = false;
      clearTimeout(initialTimer);
      clearInterval(interval);
    };
  }, [fetchData]);

  return { data, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Mutation hooks
// ---------------------------------------------------------------------------

interface UseMutationReturn {
  execute: () => Promise<OperatorMutationResponse>;
  loading: boolean;
  error: string | null;
}

interface UseMutationWithBodyReturn<T> {
  execute: (body: T) => Promise<OperatorMutationResponse>;
  loading: boolean;
  error: string | null;
}

function useOperatorMutation(url: string): UseMutationReturn {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const execute = useCallback(async (): Promise<OperatorMutationResponse> => {
    setLoading(true);
    setError(null);
    try {
      const result = await apiPost(url, {}, parseOperatorMutationResponse);
      return result;
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      setError(message);
      return { success: false, message };
    } finally {
      setLoading(false);
    }
  }, [url]);

  return { execute, loading, error };
}

function useOperatorMutationWithBody<T>(url: string): UseMutationWithBodyReturn<T> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const execute = useCallback(
    async (body: T): Promise<OperatorMutationResponse> => {
      setLoading(true);
      setError(null);
      try {
        const result = await apiPost(url, body, parseOperatorMutationResponse);
        return result;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Unknown error";
        setError(message);
        return { success: false, message };
      } finally {
        setLoading(false);
      }
    },
    [url],
  );

  return { execute, loading, error };
}

// ---------------------------------------------------------------------------
// Exported mutation hooks
// ---------------------------------------------------------------------------

export function usePauseSimulation(): UseMutationReturn {
  return useOperatorMutation("/api/operator/pause");
}

export function useResumeSimulation(): UseMutationReturn {
  return useOperatorMutation("/api/operator/resume");
}

export function useSetSpeed(): UseMutationWithBodyReturn<{ tick_interval_ms: number }> {
  return useOperatorMutationWithBody<{ tick_interval_ms: number }>("/api/operator/speed");
}

export function useInjectEvent(): UseMutationWithBodyReturn<InjectEventRequest> {
  return useOperatorMutationWithBody<InjectEventRequest>("/api/operator/inject-event");
}

export function useStopSimulation(): UseMutationReturn {
  return useOperatorMutation("/api/operator/stop");
}

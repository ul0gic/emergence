/**
 * REST API hooks for querying the Emergence backend.
 *
 * All responses are validated through Zod schemas. Includes debouncing
 * for search queries and proper error handling.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import type {
  AgentDetailResponse,
  AgentListItem,
  Event,
  LocationDetailResponse,
  LocationListItem,
  WorldSnapshot,
} from "../types/generated/index.ts";
import {
  parseAgentDetail,
  parseAgentsResponse,
  parseEventsResponse,
  parseLocationDetail,
  parseLocationsResponse,
  parseWorldSnapshot,
} from "../types/schemas.ts";

// ---------------------------------------------------------------------------
// Generic fetch wrapper
// ---------------------------------------------------------------------------

async function apiFetch<T>(url: string, parser: (data: unknown) => T): Promise<T> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`API error: ${response.status} ${response.statusText}`);
  }
  const raw: unknown = await response.json();
  return parser(raw);
}

// ---------------------------------------------------------------------------
// Hook: useWorldSnapshot
// ---------------------------------------------------------------------------

interface UseWorldSnapshotReturn {
  data: WorldSnapshot | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useWorldSnapshot(): UseWorldSnapshotReturn {
  const [data, setData] = useState<WorldSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await apiFetch("/api/world", parseWorldSnapshot);
      setData(result);
    } catch (err) {
      // World endpoint may return minimal data before simulation starts.
      // Fallback to treating missing fields gracefully.
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { data, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Hook: useAgents
// ---------------------------------------------------------------------------

type AgentStatusFilter = "alive" | "dead" | "all";

interface UseAgentsReturn {
  agents: AgentListItem[];
  loading: boolean;
  error: string | null;
  refetch: (status?: AgentStatusFilter) => void;
}

export function useAgents(initialStatus: AgentStatusFilter = "all"): UseAgentsReturn {
  const [agents, setAgents] = useState<AgentListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(
    async (status: AgentStatusFilter = initialStatus) => {
      setLoading(true);
      setError(null);
      try {
        const url = `/api/agents?status=${encodeURIComponent(status)}`;
        const result = await apiFetch(url, parseAgentsResponse);
        setAgents(result.agents);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Unknown error");
      } finally {
        setLoading(false);
      }
    },
    [initialStatus],
  );

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { agents, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Hook: useAgentDetail
// ---------------------------------------------------------------------------

interface UseAgentDetailReturn {
  data: AgentDetailResponse | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useAgentDetail(agentId: string | null): UseAgentDetailReturn {
  const [data, setData] = useState<AgentDetailResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!agentId) {
      setData(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const url = `/api/agents/${encodeURIComponent(agentId)}`;
      const result = await apiFetch(url, parseAgentDetail);
      setData(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, [agentId]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { data, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Hook: useLocations
// ---------------------------------------------------------------------------

interface UseLocationsReturn {
  locations: LocationListItem[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useLocations(): UseLocationsReturn {
  const [locations, setLocations] = useState<LocationListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await apiFetch("/api/locations", parseLocationsResponse);
      setLocations(result.locations);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { locations, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Hook: useLocationDetail
// ---------------------------------------------------------------------------

interface UseLocationDetailReturn {
  data: LocationDetailResponse | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useLocationDetail(locationId: string | null): UseLocationDetailReturn {
  const [data, setData] = useState<LocationDetailResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!locationId) {
      setData(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const url = `/api/locations/${encodeURIComponent(locationId)}`;
      const result = await apiFetch(url, parseLocationDetail);
      setData(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, [locationId]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { data, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Hook: useEvents
// ---------------------------------------------------------------------------

interface UseEventsParams {
  tick?: number;
  agentId?: string;
  limit?: number;
}

interface UseEventsReturn {
  events: Event[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useEvents(params: UseEventsParams = {}): UseEventsReturn {
  const [events, setEvents] = useState<Event[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const paramsRef = useRef(params);
  paramsRef.current = params;

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const searchParams = new URLSearchParams();
      if (paramsRef.current.tick !== undefined) {
        searchParams.set("tick", String(paramsRef.current.tick));
      }
      if (paramsRef.current.agentId) {
        searchParams.set("agent_id", paramsRef.current.agentId);
      }
      if (paramsRef.current.limit !== undefined) {
        searchParams.set("limit", String(paramsRef.current.limit));
      }
      const url = `/api/events?${searchParams.toString()}`;
      const result = await apiFetch(url, parseEventsResponse);
      setEvents(result.events);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { events, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Debounced search helper
// ---------------------------------------------------------------------------

export function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedValue(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);

  return debouncedValue;
}

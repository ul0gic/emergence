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
  BeliefEvent,
  BeliefSystem,
  BeliefsResponse,
  CrimeStats,
  DecisionRecord,
  EconomicClassification,
  Event,
  FamilyStats,
  GovernanceInfo,
  LocationDetailResponse,
  LocationListItem,
  Route,
  WorldSnapshot,
} from "../types/generated/index.ts";
import {
  parseAgentDetail,
  parseAgentsResponse,
  parseBeliefsResponse,
  parseCrimeResponse,
  parseDecisionsResponse,
  parseEventsResponse,
  parseFamiliesResponse,
  parseGovernanceResponse,
  parseLocationDetail,
  parseLocationsResponse,
  parseRoutesResponse,
  parseSocialEconomyResponse,
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
// Hook: useRoutes
// ---------------------------------------------------------------------------

interface UseRoutesReturn {
  routes: Route[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useRoutes(): UseRoutesReturn {
  const [routes, setRoutes] = useState<Route[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await apiFetch("/api/routes", parseRoutesResponse);
      setRoutes(result.routes);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { routes, loading, error, refetch: fetchData };
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
// Hook: useDecisions (Phase 9.3 — LLM Decision Viewer)
// ---------------------------------------------------------------------------

interface UseDecisionsParams {
  agentId?: string | null;
  tick?: number;
  limit?: number;
}

interface UseDecisionsReturn {
  decisions: DecisionRecord[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

export function useDecisions(params: UseDecisionsParams = {}): UseDecisionsReturn {
  const [decisions, setDecisions] = useState<DecisionRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const paramsRef = useRef(params);
  paramsRef.current = params;

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const searchParams = new URLSearchParams();
      if (paramsRef.current.agentId) {
        searchParams.set("agent_id", paramsRef.current.agentId);
      }
      if (paramsRef.current.tick !== undefined) {
        searchParams.set("tick", String(paramsRef.current.tick));
      }
      searchParams.set("limit", String(paramsRef.current.limit ?? 200));
      const url = `/api/decisions?${searchParams.toString()}`;
      const result = await apiFetch(url, parseDecisionsResponse);
      setDecisions(result.decisions);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { decisions, loading, error, refetch: fetchData };
}

// ---------------------------------------------------------------------------
// Hook: useSocialConstructs (Phase 9.7 — Social Constructs Wiring)
// ---------------------------------------------------------------------------

export interface SocialConstructsData {
  beliefSystems: BeliefSystem[];
  beliefEvents: BeliefEvent[];
  governance: GovernanceInfo | null;
  familyStats: FamilyStats | null;
  economicClassification: EconomicClassification | null;
  crimeStats: CrimeStats | null;
  loading: boolean;
  error: string | null;
}

interface UseSocialConstructsReturn extends SocialConstructsData {
  refetch: () => void;
}

export function useSocialConstructs(): UseSocialConstructsReturn {
  const [beliefSystems, setBeliefSystems] = useState<BeliefSystem[]>([]);
  const [beliefEvents, setBeliefEvents] = useState<BeliefEvent[]>([]);
  const [governance, setGovernance] = useState<GovernanceInfo | null>(null);
  const [familyStats, setFamilyStats] = useState<FamilyStats | null>(null);
  const [economicClassification, setEconomicClassification] =
    useState<EconomicClassification | null>(null);
  const [crimeStats, setCrimeStats] = useState<CrimeStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);

    // Fetch all 5 endpoints in parallel. Each endpoint may fail independently
    // (e.g., no data yet) so we use allSettled and extract what succeeds.
    const [beliefsResult, governanceResult, familiesResult, economyResult, crimeResult] =
      await Promise.allSettled([
        apiFetch("/api/social/beliefs", parseBeliefsResponse),
        apiFetch("/api/social/governance", parseGovernanceResponse),
        apiFetch("/api/social/families", parseFamiliesResponse),
        apiFetch("/api/social/economy", parseSocialEconomyResponse),
        apiFetch("/api/social/crime", parseCrimeResponse),
      ]);

    if (beliefsResult.status === "fulfilled") {
      const data: BeliefsResponse = beliefsResult.value;
      setBeliefSystems(data.belief_systems);
      setBeliefEvents(data.belief_events);
    }

    if (governanceResult.status === "fulfilled") {
      setGovernance(governanceResult.value);
    }

    if (familiesResult.status === "fulfilled") {
      setFamilyStats(familiesResult.value);
    }

    if (economyResult.status === "fulfilled") {
      setEconomicClassification(economyResult.value);
    }

    if (crimeResult.status === "fulfilled") {
      setCrimeStats(crimeResult.value);
    }

    // Report error only if ALL endpoints failed.
    const allFailed = [beliefsResult, governanceResult, familiesResult, economyResult, crimeResult]
      .every((r) => r.status === "rejected");
    if (allFailed) {
      setError("Failed to fetch social construct data");
    }

    setLoading(false);
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return {
    beliefSystems,
    beliefEvents,
    governance,
    familyStats,
    economicClassification,
    crimeStats,
    loading,
    error,
    refetch: fetchData,
  };
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

/**
 * App.tsx -- Main dashboard layout with tabbed panels.
 *
 * The Observer Dashboard is a single-page application with a header showing
 * global metrics and a tab bar for switching between panels. All data flows
 * from the WebSocket hook (live ticks) and REST API hooks (on-demand queries).
 *
 * Phase 9.8: Toast notifications, simulation health bar, keyboard shortcuts
 * overlay, dark/light mode toggle, data export.
 * Phase 10.3.4: Population critical alert banner.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import AgentInspector from "./components/AgentInspector.tsx";
import DecisionViewer from "./components/DecisionViewer.tsx";
import DiscoveryLog from "./components/DiscoveryLog.tsx";
import EconomyMonitor from "./components/EconomyMonitor.tsx";
import OperatorControls from "./components/OperatorControls.tsx";
import PopulationTracker from "./components/PopulationTracker.tsx";
import SocialConstructs from "./components/SocialConstructs.tsx";
import SocialGraph from "./components/SocialGraph.tsx";
import Timeline from "./components/Timeline.tsx";
import { Badge } from "./components/ui/badge.tsx";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./components/ui/tooltip.tsx";
import WorldMap from "./components/WorldMap.tsx";
import {
  useAgentDetail,
  useAgents,
  useEvents,
  useLocationDetail,
  useLocations,
  useRoutes,
  useWorldSnapshot,
} from "./hooks/useApi.ts";
import { useWebSocket } from "./hooks/useWebSocket.ts";
import type { ConnectionStatus } from "./hooks/useWebSocket.ts";
import { cn } from "./lib/utils.ts";
import type { Event, EventType } from "./types/generated/index.ts";
import { formatNumber, getSeasonClass } from "./utils/format.ts";

// ---------------------------------------------------------------------------
// Tab definitions
// ---------------------------------------------------------------------------

type TabId =
  | "world"
  | "agents"
  | "minds"
  | "economy"
  | "social"
  | "timeline"
  | "population"
  | "discovery"
  | "operator"
  | "constructs";

const TABS: { id: TabId; label: string }[] = [
  { id: "world", label: "World Map" },
  { id: "agents", label: "Agents" },
  { id: "minds", label: "Agent Minds" },
  { id: "economy", label: "Economy" },
  { id: "social", label: "Social" },
  { id: "timeline", label: "Timeline" },
  { id: "population", label: "Population" },
  { id: "discovery", label: "Discoveries" },
  { id: "operator", label: "Operator" },
  { id: "constructs", label: "Constructs" },
];

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

function connectionStatusLabel(status: ConnectionStatus): string {
  switch (status) {
    case "connected":
      return "LIVE";
    case "connecting":
      return "CONNECTING";
    case "reconnecting":
      return "RECONNECTING";
    case "disconnected":
      return "OFFLINE";
  }
}

/** Map connection status to dot styling classes. */
function connectionDotClasses(status: ConnectionStatus): string {
  switch (status) {
    case "connected":
      return "bg-success shadow-glow-success";
    case "connecting":
    case "reconnecting":
      return "bg-warning animate-pulse-dot";
    case "disconnected":
      return "bg-danger";
  }
}

/** Map season to badge styling classes. */
function seasonBadgeClasses(seasonClass: string): string {
  switch (seasonClass) {
    case "season-spring":
      return "bg-spring/15 text-spring";
    case "season-summer":
      return "bg-summer/15 text-summer";
    case "season-autumn":
      return "bg-autumn/15 text-autumn";
    case "season-winter":
      return "bg-winter/15 text-winter";
    default:
      return "";
  }
}

// ---------------------------------------------------------------------------
// Toast system types
// ---------------------------------------------------------------------------

type ToastType = "death" | "discovery" | "trade" | "social" | "warning" | "info";

interface Toast {
  id: number;
  type: ToastType;
  message: string;
  tab?: TabId;
  exiting: boolean;
}

const TOAST_COLORS: Record<ToastType, string> = {
  death: "border-l-danger",
  discovery: "border-l-warning",
  trade: "border-l-info",
  social: "border-l-lifecycle",
  warning: "border-l-warning",
  info: "border-l-chart-1",
};

const TOAST_LABELS: Record<ToastType, string> = {
  death: "DEATH",
  discovery: "DISCOVERY",
  trade: "TRADE",
  social: "SOCIAL",
  warning: "WARNING",
  info: "INFO",
};

const TOAST_LABEL_COLORS: Record<ToastType, string> = {
  death: "text-danger",
  discovery: "text-warning",
  trade: "text-info",
  social: "text-lifecycle",
  warning: "text-warning",
  info: "text-chart-1",
};

/** Map event types to toast types. */
function eventToToastType(eventType: EventType): ToastType | null {
  switch (eventType) {
    case "AgentDied":
      return "death";
    case "KnowledgeDiscovered":
      return "discovery";
    case "TradeCompleted":
      return "trade";
    case "GroupFormed":
    case "RelationshipChanged":
      return "social";
    case "LedgerAnomaly":
      return "warning";
    default:
      return null;
  }
}

// Notable event types that generate toasts.
const NOTABLE_EVENT_TYPES: Set<EventType> = new Set([
  "AgentDied",
  "KnowledgeDiscovered",
  "TradeCompleted",
  "GroupFormed",
  "LedgerAnomaly",
]);

// ---------------------------------------------------------------------------
// Simulation health
// ---------------------------------------------------------------------------

type HealthLevel = "green" | "yellow" | "red";

interface HealthStatus {
  level: HealthLevel;
  issues: string[];
}

function computeSimulationHealth(
  population: number,
  totalDead: number,
  deathsThisTick: number,
  connectionStatus: ConnectionStatus,
): HealthStatus {
  const issues: string[] = [];

  if (connectionStatus === "disconnected") {
    issues.push("WebSocket disconnected");
  }

  if (population === 0 && totalDead > 0) {
    issues.push("All agents dead -- population extinct");
    return { level: "red", issues };
  }

  if (population <= 2 && population > 0) {
    issues.push(`Population critical: only ${population} agent${population === 1 ? "" : "s"} alive`);
  }

  if (deathsThisTick >= 3) {
    issues.push(`High death rate: ${deathsThisTick} deaths this tick`);
  }

  if (population <= 2) {
    return { level: "red", issues };
  }

  if (issues.length > 0) {
    return { level: "yellow", issues };
  }

  return { level: "green", issues: ["Simulation running normally"] };
}

const HEALTH_COLORS: Record<HealthLevel, string> = {
  green: "bg-success",
  yellow: "bg-warning",
  red: "bg-danger",
};

const HEALTH_GLOW: Record<HealthLevel, string> = {
  green: "shadow-glow-success",
  yellow: "shadow-glow-warning",
  red: "shadow-glow-danger",
};

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

const KEYBOARD_SHORTCUTS: { key: string; description: string }[] = [
  { key: "1-9, 0", description: "Switch tabs (World, Agents, ... Constructs)" },
  { key: "R", description: "Reconnect WebSocket (when offline)" },
  { key: "Space", description: "Pause / Resume simulation" },
  { key: "+", description: "Increase simulation speed" },
  { key: "-", description: "Decrease simulation speed" },
  { key: "?", description: "Toggle this help overlay" },
  { key: "D", description: "Toggle dark / light mode" },
  { key: "E", description: "Export simulation data" },
  { key: "Esc", description: "Close overlays" },
];

// ---------------------------------------------------------------------------
// Main App Component
// ---------------------------------------------------------------------------

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>("world");
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [selectedLocationId, setSelectedLocationId] = useState<string | null>(null);

  // Toast state.
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastIdRef = useRef(0);
  const seenEventIdsRef = useRef(new Set<string>());

  // Keyboard shortcuts overlay.
  const [showShortcuts, setShowShortcuts] = useState(false);

  // Dark/light mode.
  const [lightMode, setLightMode] = useState(() => {
    try {
      return localStorage.getItem("emergence-theme") === "light";
    } catch {
      return false;
    }
  });

  // Population alert banner dismiss state.
  const [alertDismissed, setAlertDismissed] = useState(false);

  // WebSocket for live tick streaming.
  const { status, latestTick, tickHistory, reconnect } = useWebSocket();

  // REST API hooks.
  const { data: worldSnapshot, refetch: refetchWorld } = useWorldSnapshot();
  const { agents, refetch: refetchAgents } = useAgents();
  const { data: agentDetail } = useAgentDetail(selectedAgentId);
  const { locations, refetch: refetchLocations } = useLocations();
  const { routes, refetch: refetchRoutes } = useRoutes();
  const { data: locationDetail } = useLocationDetail(selectedLocationId);
  const { events, refetch: refetchEvents } = useEvents({ limit: 500 });

  // Refresh REST data when new ticks arrive.
  useEffect(() => {
    if (latestTick) {
      refetchWorld();
      refetchAgents();
      refetchLocations();
      refetchRoutes();
      refetchEvents();
    }
    // Only re-run when tick number changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [latestTick?.tick]);

  // Apply light/dark mode class.
  useEffect(() => {
    const root = document.getElementById("root");
    if (!root) return;
    if (lightMode) {
      root.classList.add("light");
    } else {
      root.classList.remove("light");
    }
    try {
      localStorage.setItem("emergence-theme", lightMode ? "light" : "dark");
    } catch {
      // localStorage may be unavailable.
    }
  }, [lightMode]);

  // Build helper maps.
  const agentNames = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agents) {
      map.set(a.id, a.name);
    }
    return map;
  }, [agents]);

  const locationNames = useMemo(() => {
    const map = new Map<string, string>();
    for (const l of locations) {
      map.set(l.id, l.name);
    }
    return map;
  }, [locations]);

  // Build relationships map from agent details.
  const relationships = useMemo(() => {
    const map = new Map<string, Record<string, string | undefined>>();
    if (agentDetail?.state) {
      map.set(agentDetail.state.agent_id, agentDetail.state.relationships);
    }
    return map;
  }, [agentDetail]);

  // Display values.
  const currentTick = latestTick?.tick ?? worldSnapshot?.tick ?? 0;
  const currentSeason = latestTick?.season ?? worldSnapshot?.season ?? "Spring";
  const currentWeather = latestTick?.weather ?? worldSnapshot?.weather ?? "Clear";
  const currentPopulation = latestTick?.agents_alive ?? worldSnapshot?.population?.total_alive ?? 0;
  const currentEra = worldSnapshot?.era ?? "Primitive";
  const totalDead = worldSnapshot?.population?.total_dead ?? 0;
  const deathsThisTick = latestTick?.deaths_this_tick ?? worldSnapshot?.population?.deaths_this_tick ?? 0;

  // Simulation health.
  const health = useMemo(
    () => computeSimulationHealth(currentPopulation, totalDead, deathsThisTick, status),
    [currentPopulation, totalDead, deathsThisTick, status],
  );

  // Population alert: show when critical and not dismissed.
  const showPopulationAlert = currentPopulation > 0 && currentPopulation <= 2 && !alertDismissed;

  // Re-show alert if population drops further.
  const prevPopRef = useRef(currentPopulation);
  useEffect(() => {
    if (currentPopulation < prevPopRef.current && currentPopulation <= 2) {
      setAlertDismissed(false);
    }
    prevPopRef.current = currentPopulation;
  }, [currentPopulation]);

  // ---------------------------------------------------------------------------
  // Toast management
  // ---------------------------------------------------------------------------

  const addToast = useCallback((type: ToastType, message: string, tab?: TabId) => {
    toastIdRef.current += 1;
    const id = toastIdRef.current;
    setToasts((prev) => {
      const next = [...prev, { id, type, message, tab, exiting: false }];
      // Cap at 5 visible toasts.
      if (next.length > 5) return next.slice(next.length - 5);
      return next;
    });

    // Auto-dismiss after 5 seconds.
    setTimeout(() => {
      setToasts((prev) => prev.map((t) => (t.id === id ? { ...t, exiting: true } : t)));
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, 300);
    }, 5000);
  }, []);

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.map((t) => (t.id === id ? { ...t, exiting: true } : t)));
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 300);
  }, []);

  // Generate toasts from events.
  useEffect(() => {
    for (const event of events) {
      if (seenEventIdsRef.current.has(event.id)) continue;
      seenEventIdsRef.current.add(event.id);

      if (!NOTABLE_EVENT_TYPES.has(event.event_type)) continue;

      const toastType = eventToToastType(event.event_type);
      if (!toastType) continue;

      const message = formatEventMessage(event, agentNames, locationNames);
      const tab = eventToTab(event.event_type);
      addToast(toastType, message, tab);
    }

    // Prune seen set to avoid unbounded growth.
    if (seenEventIdsRef.current.size > 2000) {
      const arr = [...seenEventIdsRef.current];
      seenEventIdsRef.current = new Set(arr.slice(arr.length - 1000));
    }
  }, [events, agentNames, locationNames, addToast]);

  // ---------------------------------------------------------------------------
  // Data export
  // ---------------------------------------------------------------------------

  const handleExport = useCallback(() => {
    const exportData = {
      exported_at: new Date().toISOString(),
      tick: currentTick,
      era: currentEra,
      season: currentSeason,
      weather: currentWeather,
      worldSnapshot,
      agents,
      events: events.slice(0, 1000), // Limit to avoid huge files.
      tickHistory: tickHistory.slice(0, 500),
    };

    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `emergence-run-T${currentTick}-${new Date().toISOString().slice(0, 10)}.json`;
    link.click();
    URL.revokeObjectURL(url);

    addToast("info", "Simulation data exported", undefined);
  }, [currentTick, currentEra, currentSeason, currentWeather, worldSnapshot, agents, events, tickHistory, addToast]);

  const handleSelectLocation = useCallback((locationId: string) => {
    setSelectedLocationId((prev) => (prev === locationId ? null : locationId));
  }, []);

  // ---------------------------------------------------------------------------
  // Keyboard shortcuts
  // ---------------------------------------------------------------------------

  useEffect(() => {
    function handleKeydown(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      // Close overlays with Escape.
      if (e.key === "Escape") {
        setShowShortcuts(false);
        e.preventDefault();
        return;
      }

      // Toggle shortcuts overlay.
      if (e.key === "?") {
        setShowShortcuts((prev) => !prev);
        e.preventDefault();
        return;
      }

      // Dark/light toggle.
      if (e.key === "d" || e.key === "D") {
        setLightMode((prev) => !prev);
        e.preventDefault();
        return;
      }

      // Export.
      if (e.key === "e" || e.key === "E") {
        handleExport();
        e.preventDefault();
        return;
      }

      // Tab switching.
      const tabKeys: Record<string, TabId> = {
        "1": "world",
        "2": "agents",
        "3": "minds",
        "4": "economy",
        "5": "social",
        "6": "timeline",
        "7": "population",
        "8": "discovery",
        "9": "operator",
        "0": "constructs",
      };
      const tab = tabKeys[e.key];
      if (tab) {
        setActiveTab(tab);
        e.preventDefault();
        return;
      }

      // Reconnect.
      if (e.key === "r" && status !== "connected") {
        reconnect();
        e.preventDefault();
      }
    }

    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  }, [status, reconnect, handleExport]);

  return (
    <TooltipProvider>
    <div className="flex flex-col h-screen overflow-hidden">
      {/* Header */}
      <header className="flex items-center justify-between px-lg py-sm bg-bg-secondary border-b border-border-primary h-12 shrink-0">
        <div className="flex items-center gap-md">
          <h1 className="text-base font-semibold text-text-accent font-mono">EMERGENCE</h1>
          {/* Simulation health indicator */}
          <Tooltip>
            <TooltipTrigger asChild>
              <span
                className={cn(
                  "w-2.5 h-2.5 rounded-full inline-block cursor-help",
                  HEALTH_COLORS[health.level],
                  HEALTH_GLOW[health.level],
                  health.level === "red" && "animate-pulse-dot",
                )}
              />
            </TooltipTrigger>
            <TooltipContent side="bottom" align="start">
              <div className="text-text-secondary uppercase tracking-wide text-2xs mb-1">
                Simulation Health
              </div>
              {health.issues.map((issue, i) => (
                <div key={i} className="text-text-primary">{issue}</div>
              ))}
            </TooltipContent>
          </Tooltip>
        </div>
        <div className="flex items-center gap-lg">
          <div className="flex items-center gap-xs font-mono text-xs">
            <span className="text-text-secondary">TICK</span>
            <span className="text-text-primary font-semibold">{formatNumber(currentTick)}</span>
          </div>
          <div className="flex items-center gap-xs font-mono text-xs">
            <span className="text-text-secondary">ERA</span>
            <Badge variant="lifecycle">{currentEra}</Badge>
          </div>
          <div className="flex items-center gap-xs font-mono text-xs">
            <span className="text-text-secondary">SEASON</span>
            <span
              className={cn(
                "inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold",
                seasonBadgeClasses(getSeasonClass(currentSeason)),
              )}
            >
              {currentSeason}
            </span>
          </div>
          <div className="flex items-center gap-xs font-mono text-xs">
            <span className="text-text-secondary">WEATHER</span>
            <span className="text-text-primary font-semibold">{currentWeather}</span>
          </div>
          <div className="flex items-center gap-xs font-mono text-xs">
            <span className="text-text-secondary">POP</span>
            <span className={cn(
              "font-semibold",
              currentPopulation <= 2 && currentPopulation > 0 ? "text-danger" : "text-text-primary",
            )}>
              {formatNumber(currentPopulation)}
            </span>
          </div>
          <div className="flex items-center gap-xs font-mono text-2xs">
            <span className={cn("w-2 h-2 rounded-full", connectionDotClasses(status))} />
            <span>{connectionStatusLabel(status)}</span>
          </div>

          {/* Dark/Light toggle */}
          <button
            className="bg-transparent border border-border-primary rounded-sm px-sm py-xs text-2xs font-mono text-text-secondary cursor-pointer hover:text-text-primary hover:border-border-primary transition-colors"
            onClick={() => setLightMode((prev) => !prev)}
            title={lightMode ? "Switch to dark mode" : "Switch to light mode"}
          >
            {lightMode ? "DARK" : "LIGHT"}
          </button>

          {/* Export button */}
          <button
            className="bg-transparent border border-border-primary rounded-sm px-sm py-xs text-2xs font-mono text-text-secondary cursor-pointer hover:text-text-primary hover:border-border-primary transition-colors"
            onClick={handleExport}
            title="Export simulation data as JSON"
          >
            EXPORT
          </button>

          {/* Shortcuts help */}
          <button
            className="bg-transparent border border-border-primary rounded-sm w-6 h-6 flex items-center justify-center text-2xs font-mono text-text-secondary cursor-pointer hover:text-text-primary hover:border-border-primary transition-colors"
            onClick={() => setShowShortcuts((prev) => !prev)}
            title="Keyboard shortcuts (?)"
          >
            ?
          </button>
        </div>
      </header>

      {/* Population critical alert banner */}
      {showPopulationAlert && (
        <div className="flex items-center justify-between px-lg py-sm bg-danger/15 border-b border-danger/30 shrink-0">
          <div className="flex items-center gap-sm font-mono text-xs">
            <span className="w-2 h-2 rounded-full bg-danger animate-pulse-dot" />
            <span className="text-danger font-semibold uppercase tracking-wide">Population Critical</span>
            <span className="text-text-primary">
              Only {currentPopulation} agent{currentPopulation === 1 ? "" : "s"} alive. Inject agents via Operator Controls or enable auto-recovery.
            </span>
          </div>
          <button
            className="bg-transparent border border-danger/30 rounded-sm px-sm py-xs text-2xs font-mono text-danger cursor-pointer hover:bg-danger/10 transition-colors"
            onClick={() => setAlertDismissed(true)}
          >
            DISMISS
          </button>
        </div>
      )}

      {/* Tab bar */}
      <nav className="flex bg-bg-secondary border-b border-border-primary px-lg shrink-0 overflow-x-auto">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            className={cn(
              "px-lg py-sm text-xs font-mono bg-transparent border-0 border-b-2 border-b-transparent cursor-pointer whitespace-nowrap transition-colors duration-150",
              activeTab === tab.id
                ? "text-text-accent border-b-text-accent"
                : "text-text-secondary hover:text-text-primary",
            )}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {/* Content */}
      <main className="flex-1 min-h-0 overflow-auto p-lg flex flex-col">
        {activeTab === "world" && (
          <WorldMap
            locations={locations}
            agents={agents}
            routes={routes}
            events={events}
            currentTick={currentTick}
            selectedLocationId={selectedLocationId}
            locationDetail={locationDetail}
            agentNames={agentNames}
            onSelectLocation={handleSelectLocation}
            onSelectAgent={(agentId: string) => {
              setSelectedAgentId(agentId);
              setActiveTab("agents");
            }}
          />
        )}
        {activeTab === "agents" && (
          <AgentInspector
            agents={agents}
            onSelectAgent={setSelectedAgentId}
            selectedAgentId={selectedAgentId}
            agentDetail={agentDetail}
            events={events}
            locationNames={locationNames}
          />
        )}
        {activeTab === "minds" && (
          <DecisionViewer agents={agents} />
        )}
        {activeTab === "economy" && (
          <EconomyMonitor
            worldSnapshot={worldSnapshot}
            tickHistory={tickHistory}
            agents={agents}
          />
        )}
        {activeTab === "social" && (
          <SocialGraph agents={agents} relationships={relationships} />
        )}
        {activeTab === "timeline" && (
          <Timeline
            events={events}
            agentNames={agentNames}
            locationNames={locationNames}
          />
        )}
        {activeTab === "population" && (
          <PopulationTracker
            populationStats={worldSnapshot?.population ?? null}
            agents={agents}
            tickHistory={tickHistory}
          />
        )}
        {activeTab === "discovery" && (
          <DiscoveryLog
            worldSnapshot={worldSnapshot}
            events={events}
            agents={agents}
            agentNames={agentNames}
          />
        )}
        {activeTab === "operator" && (
          <OperatorControls connectionStatus={status} />
        )}
        {activeTab === "constructs" && <SocialConstructs currentTick={currentTick} />}
      </main>

      {/* Toast notifications */}
      <div className="fixed top-14 right-4 z-50 flex flex-col gap-sm w-80 pointer-events-none">
        {toasts.map((toast) => (
          <div
            key={toast.id}
            className={cn(
              "pointer-events-auto bg-bg-elevated border border-border-primary border-l-4 rounded-sm px-md py-sm font-mono text-xs cursor-pointer",
              TOAST_COLORS[toast.type],
              toast.exiting ? "animate-toast-out" : "animate-toast-in",
            )}
            onClick={() => {
              if (toast.tab) setActiveTab(toast.tab);
              dismissToast(toast.id);
            }}
          >
            <div className={cn("text-2xs uppercase tracking-wide font-semibold mb-0.5", TOAST_LABEL_COLORS[toast.type])}>
              {TOAST_LABELS[toast.type]}
            </div>
            <div className="text-text-primary leading-snug">{toast.message}</div>
          </div>
        ))}
      </div>

      {/* Keyboard shortcuts overlay */}
      {showShortcuts && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
          onClick={() => setShowShortcuts(false)}
        >
          <div
            className="bg-bg-secondary border border-border-primary rounded-md p-xl max-w-md w-full shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-md">
              <h2 className="text-sm font-semibold text-text-accent font-mono uppercase tracking-wide">
                Keyboard Shortcuts
              </h2>
              <button
                className="bg-transparent border-0 text-text-muted cursor-pointer hover:text-text-primary text-lg leading-none"
                onClick={() => setShowShortcuts(false)}
              >
                x
              </button>
            </div>
            <div className="flex flex-col gap-sm">
              {KEYBOARD_SHORTCUTS.map((shortcut) => (
                <div key={shortcut.key} className="flex items-center justify-between text-xs font-mono">
                  <span className="text-text-secondary">{shortcut.description}</span>
                  <span className="bg-bg-tertiary border border-border-primary rounded-sm px-sm py-xs text-text-accent text-2xs">
                    {shortcut.key}
                  </span>
                </div>
              ))}
            </div>
            <div className="mt-md text-2xs text-text-muted font-mono text-center">
              Press Esc or click outside to close
            </div>
          </div>
        </div>
      )}
    </div>
    </TooltipProvider>
  );
}

// ---------------------------------------------------------------------------
// Event message formatting for toasts
// ---------------------------------------------------------------------------

function formatEventMessage(
  event: Event,
  agentNames: Map<string, string>,
  locationNames: Map<string, string>,
): string {
  const agentName = event.agent_id ? (agentNames.get(event.agent_id) ?? "Unknown") : "Unknown";
  const locationName = event.location_id ? (locationNames.get(event.location_id) ?? "") : "";

  switch (event.event_type) {
    case "AgentDied": {
      const details = event.details as Record<string, unknown> | null;
      const cause = details && typeof details === "object" && "cause" in details
        ? String(details.cause)
        : "unknown causes";
      return `${agentName} died of ${cause}${locationName ? ` at ${locationName}` : ""}`;
    }
    case "KnowledgeDiscovered": {
      const details = event.details as Record<string, unknown> | null;
      const knowledge = details && typeof details === "object" && "knowledge" in details
        ? String(details.knowledge)
        : "something new";
      return `${agentName} discovered: ${knowledge}`;
    }
    case "TradeCompleted":
      return `Trade completed${locationName ? ` at ${locationName}` : ""}`;
    case "GroupFormed":
      return `${agentName} formed a new group${locationName ? ` at ${locationName}` : ""}`;
    case "LedgerAnomaly":
      return "Ledger anomaly detected -- conservation law may be violated";
    default:
      return `${event.event_type}: ${agentName}`;
  }
}

/** Map event type to the relevant dashboard tab. */
function eventToTab(eventType: EventType): TabId | undefined {
  switch (eventType) {
    case "AgentDied":
      return "population";
    case "KnowledgeDiscovered":
      return "discovery";
    case "TradeCompleted":
      return "economy";
    case "GroupFormed":
    case "RelationshipChanged":
      return "social";
    case "LedgerAnomaly":
      return "economy";
    default:
      return undefined;
  }
}

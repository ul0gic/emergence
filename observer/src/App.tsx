/**
 * App.tsx -- Main dashboard layout with tabbed panels.
 *
 * The Observer Dashboard is a single-page application with a header showing
 * global metrics and a tab bar for switching between panels. All data flows
 * from the WebSocket hook (live ticks) and REST API hooks (on-demand queries).
 *
 * When the backend is not running, the dashboard falls back to mock data
 * so all panels still render.
 */
import { useCallback, useEffect, useMemo, useState } from "react";

import AgentInspector from "./components/AgentInspector.tsx";
import DiscoveryLog from "./components/DiscoveryLog.tsx";
import EconomyMonitor from "./components/EconomyMonitor.tsx";
import OperatorControls from "./components/OperatorControls.tsx";
import PopulationTracker from "./components/PopulationTracker.tsx";
import SocialConstructs from "./components/SocialConstructs.tsx";
import SocialGraph from "./components/SocialGraph.tsx";
import Timeline from "./components/Timeline.tsx";
import WorldMap from "./components/WorldMap.tsx";
import {
  useAgentDetail,
  useAgents,
  useEvents,
  useLocations,
  useWorldSnapshot,
} from "./hooks/useApi.ts";
import { useWebSocket } from "./hooks/useWebSocket.ts";
import type { ConnectionStatus } from "./hooks/useWebSocket.ts";
import { cn } from "./lib/utils.ts";
import { formatNumber, getSeasonClass } from "./utils/format.ts";
import { MOCK_ROUTES } from "./utils/mockData.ts";

type TabId =
  | "world"
  | "agents"
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
  { id: "economy", label: "Economy" },
  { id: "social", label: "Social" },
  { id: "timeline", label: "Timeline" },
  { id: "population", label: "Population" },
  { id: "discovery", label: "Discoveries" },
  { id: "operator", label: "Operator" },
  { id: "constructs", label: "Constructs" },
];

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

export default function App() {
  const [activeTab, setActiveTab] = useState<TabId>("world");
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);

  // WebSocket for live tick streaming.
  const { status, latestTick, tickHistory, reconnect } = useWebSocket();

  // REST API hooks.
  const { data: worldSnapshot, refetch: refetchWorld } = useWorldSnapshot();
  const { agents, refetch: refetchAgents } = useAgents();
  const { data: agentDetail } = useAgentDetail(selectedAgentId);
  const { locations, refetch: refetchLocations } = useLocations();
  const { events, refetch: refetchEvents } = useEvents({ limit: 500 });

  // Determine if we should use mock data (no backend).
  const useMock = agents.length === 0 && locations.length === 0 && status !== "connected";

  // Refresh REST data when new ticks arrive.
  useEffect(() => {
    if (latestTick) {
      refetchWorld();
      refetchAgents();
      refetchLocations();
      refetchEvents();
    }
    // Only re-run when tick number changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [latestTick?.tick]);

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

  const handleSelectLocation = useCallback((_locationId: string) => {
    // Future: open location detail drawer.
  }, []);

  // Keyboard shortcuts.
  useEffect(() => {
    function handleKeydown(e: KeyboardEvent) {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      const tabKeys: Record<string, TabId> = {
        "1": "world",
        "2": "agents",
        "3": "economy",
        "4": "social",
        "5": "timeline",
        "6": "population",
        "7": "discovery",
        "8": "operator",
        "9": "constructs",
      };
      const tab = tabKeys[e.key];
      if (tab) {
        setActiveTab(tab);
        e.preventDefault();
      }

      if (e.key === "r" && status !== "connected") {
        reconnect();
        e.preventDefault();
      }
    }

    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  }, [status, reconnect]);

  return (
    <div className="flex flex-col h-screen overflow-hidden">
      {/* Header */}
      <header className="flex items-center justify-between px-lg py-sm bg-bg-secondary border-b border-border-primary h-12 shrink-0">
        <h1 className="text-base font-semibold text-text-accent font-mono">EMERGENCE</h1>
        <div className="flex items-center gap-lg">
          <div className="flex items-center gap-xs font-mono text-xs">
            <span className="text-text-secondary">TICK</span>
            <span className="text-text-primary font-semibold">{formatNumber(currentTick)}</span>
          </div>
          <div className="flex items-center gap-xs font-mono text-xs">
            <span className="text-text-secondary">ERA</span>
            <span className="inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold bg-lifecycle/15 text-lifecycle">
              {currentEra}
            </span>
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
            <span className="text-text-primary font-semibold">
              {formatNumber(currentPopulation)}
            </span>
          </div>
          <div className="flex items-center gap-xs font-mono text-2xs">
            <span className={cn("w-2 h-2 rounded-full", connectionDotClasses(status))} />
            <span>{connectionStatusLabel(status)}</span>
            {useMock && <span className="text-warning ml-1">(mock)</span>}
          </div>
        </div>
      </header>

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
            routes={MOCK_ROUTES}
            onSelectLocation={handleSelectLocation}
            useMock={useMock}
          />
        )}
        {activeTab === "agents" && (
          <AgentInspector
            agents={agents}
            onSelectAgent={setSelectedAgentId}
            selectedAgentId={selectedAgentId}
            agentDetail={agentDetail}
            useMock={useMock}
          />
        )}
        {activeTab === "economy" && (
          <EconomyMonitor
            worldSnapshot={worldSnapshot}
            tickHistory={tickHistory}
            useMock={useMock}
          />
        )}
        {activeTab === "social" && (
          <SocialGraph agents={agents} relationships={relationships} useMock={useMock} />
        )}
        {activeTab === "timeline" && (
          <Timeline
            events={events}
            agentNames={agentNames}
            locationNames={locationNames}
            useMock={useMock}
          />
        )}
        {activeTab === "population" && (
          <PopulationTracker
            populationStats={worldSnapshot?.population ?? null}
            agents={agents}
            tickHistory={tickHistory}
            useMock={useMock}
          />
        )}
        {activeTab === "discovery" && (
          <DiscoveryLog
            worldSnapshot={worldSnapshot}
            events={events}
            agents={agents}
            agentNames={agentNames}
            useMock={useMock}
          />
        )}
        {activeTab === "operator" && (
          <OperatorControls connectionStatus={status} useMock={useMock} />
        )}
        {activeTab === "constructs" && <SocialConstructs useMock={useMock} />}
      </main>
    </div>
  );
}

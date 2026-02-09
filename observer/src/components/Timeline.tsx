/**
 * Timeline Panel (Task 4.5.5)
 *
 * Scrollable event history with filtering by event type, agent, location.
 * Color-coded event entries. Expandable details. Auto-scroll to latest
 * with pause button.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { cn } from "../lib/utils.ts";
import type { Event, EventType } from "../types/generated/index.ts";
import { formatNumber, formatTick, getEventCategory } from "../utils/format.ts";
import { MOCK_EVENTS } from "../utils/mockData.ts";

interface TimelineProps {
  events: Event[];
  agentNames: Map<string, string>;
  locationNames: Map<string, string>;
  useMock?: boolean;
}

const EVENT_CATEGORIES = new Map<string, EventType[]>([
  ["lifecycle", ["AgentBorn", "AgentDied"]],
  [
    "economy",
    ["ResourceGathered", "ResourceConsumed", "TradeCompleted", "TradeFailed", "LedgerAnomaly"],
  ],
  ["social", ["MessageSent", "GroupFormed", "RelationshipChanged"]],
  [
    "world",
    [
      "StructureBuilt",
      "StructureDestroyed",
      "StructureRepaired",
      "RouteImproved",
      "LocationDiscovered",
    ],
  ],
  ["knowledge", ["KnowledgeDiscovered", "KnowledgeTaught"]],
  ["system", ["TickStart", "TickEnd", "ActionSubmitted", "ActionSucceeded", "ActionRejected"]],
  ["environment", ["WeatherChanged", "SeasonChanged"]],
]);

const CATEGORY_LABELS = new Map<string, string>([
  ["lifecycle", "Lifecycle"],
  ["economy", "Economy"],
  ["social", "Social"],
  ["world", "World"],
  ["knowledge", "Knowledge"],
  ["system", "System"],
  ["environment", "Environment"],
]);

/** Map event category CSS class to Tailwind color class. */
function eventCategoryColorClass(cssClass: string): string {
  switch (cssClass) {
    case "event-lifecycle":
      return "text-lifecycle";
    case "event-economy":
      return "text-economy";
    case "event-social":
      return "text-social";
    case "event-world":
      return "text-world";
    case "event-knowledge":
      return "text-knowledge";
    case "event-system":
      return "text-system";
    case "event-environment":
      return "text-environment";
    default:
      return "text-system";
  }
}

export default function Timeline({
  events: propEvents,
  agentNames: propAgentNames,
  locationNames: propLocationNames,
  useMock = false,
}: TimelineProps) {
  const [categoryFilter, setCategoryFilter] = useState<string | null>(null);
  const [searchText, setSearchText] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const listRef = useRef<HTMLDivElement>(null);

  const events = useMock ? MOCK_EVENTS : propEvents;

  const agentNames = useMemo(() => {
    if (!useMock) return propAgentNames;
    return new Map([
      ["01945c2a-3b4f-7def-8a12-bc34567890a1", "Kora"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a2", "Maren"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a3", "Dax"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a4", "Vela"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a5", "Rune"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a6", "Thane"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a7", "Lyra"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a8", "Orin"],
      ["01945c2a-3b4f-7def-8a12-bc34567890a9", "Sage"],
      ["01945c2a-3b4f-7def-8a12-bc3456789010", "Ember"],
    ]);
  }, [useMock, propAgentNames]);

  const locationNames = useMemo(() => {
    if (!useMock) return propLocationNames;
    return new Map([
      ["01945c2a-3b4f-7def-8a12-bc34567890c1", "Riverbank"],
      ["01945c2a-3b4f-7def-8a12-bc34567890c2", "Forest Edge"],
      ["01945c2a-3b4f-7def-8a12-bc34567890c3", "Open Field"],
      ["01945c2a-3b4f-7def-8a12-bc34567890c4", "Rocky Outcrop"],
    ]);
  }, [useMock, propLocationNames]);

  // Filter events.
  const filteredEvents = useMemo(() => {
    let list = events;

    // Category filter.
    if (categoryFilter) {
      const types = EVENT_CATEGORIES.get(categoryFilter) ?? [];
      list = list.filter((e) => types.includes(e.event_type));
    }

    // Exclude system tick events by default (too noisy).
    if (!categoryFilter) {
      list = list.filter((e) => e.event_type !== "TickStart" && e.event_type !== "TickEnd");
    }

    // Text search.
    if (searchText) {
      const q = searchText.toLowerCase();
      list = list.filter((e) => {
        const agentName = e.agent_id ? (agentNames.get(e.agent_id) ?? "") : "";
        const locName = e.location_id ? (locationNames.get(e.location_id) ?? "") : "";
        const details = JSON.stringify(e.details).toLowerCase();
        return (
          e.event_type.toLowerCase().includes(q) ||
          agentName.toLowerCase().includes(q) ||
          locName.toLowerCase().includes(q) ||
          details.includes(q)
        );
      });
    }

    return list;
  }, [events, categoryFilter, searchText, agentNames, locationNames]);

  // Auto-scroll to top when new events arrive.
  useEffect(() => {
    if (autoScroll && listRef.current) {
      listRef.current.scrollTop = 0;
    }
  }, [filteredEvents.length, autoScroll]);

  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const formatEventSummary = useCallback(
    (event: Event): string => {
      const agent = event.agent_id
        ? (agentNames.get(event.agent_id) ?? event.agent_id.slice(0, 8))
        : "";
      const loc = event.location_id ? (locationNames.get(event.location_id) ?? "") : "";
      const details = event.details as Record<string, unknown> | null;

      switch (event.event_type) {
        case "ResourceGathered":
          return `${agent} gathered ${(details?.quantity as number) ?? "?"} ${(details?.resource as string) ?? "?"} at ${loc}`;
        case "TradeCompleted":
          return `${agent} traded with ${agentNames.get(String(details?.agent_b ?? "")) ?? "?"}`;
        case "KnowledgeDiscovered":
          return `${agent} discovered "${(details?.knowledge as string) ?? "?"}" via ${(details?.method as string) ?? "?"}`;
        case "AgentBorn":
          return `${agent} was born at ${loc}`;
        case "AgentDied":
          return `${agent} died from ${(details?.cause as string) ?? "?"} at age ${(details?.final_age as number) ?? "?"}`;
        case "StructureBuilt":
          return `${agent} built ${(details?.structure_type as string) ?? "?"} at ${loc}`;
        case "RelationshipChanged":
          return `${agent}'s relationship with ${agentNames.get(String(details?.target ?? "")) ?? "?"} changed`;
        case "MessageSent":
          return `${agent} sent a message at ${loc}`;
        case "SeasonChanged":
          return `Season changed: ${(details?.from as string) ?? "?"} -> ${(details?.to as string) ?? "?"}`;
        case "WeatherChanged":
          return `Weather changed`;
        default: {
          const byAgent = agent ? ` by ${agent}` : "";
          const atLoc = loc ? ` at ${loc}` : "";
          return `${event.event_type}${byAgent}${atLoc}`;
        }
      }
    },
    [agentNames, locationNames],
  );

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>Timeline</span>
        <div className="flex items-center gap-sm">
          <span className="text-xs font-normal">{formatNumber(filteredEvents.length)} events</span>
          <button
            onClick={() => setAutoScroll(!autoScroll)}
            className={cn(
              "px-1.5 py-px border border-border-primary rounded-sm font-mono text-2xs cursor-pointer transition-all duration-150",
              autoScroll
                ? "bg-info/15 border-text-accent text-text-accent"
                : "bg-bg-primary text-text-secondary",
            )}
          >
            {autoScroll ? "live" : "paused"}
          </button>
        </div>
      </div>

      {/* Filters */}
      <div className="p-sm border-b border-border-primary">
        <input
          className="w-full px-md py-sm bg-bg-primary border border-border-primary rounded-sm text-text-primary font-mono text-xs outline-none focus:border-text-accent placeholder:text-text-muted mb-xs"
          placeholder="Search events..."
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
        />
        <div className="flex gap-xs">
          <button
            className={cn(
              "px-2 py-px border border-border-primary rounded-sm bg-bg-primary font-mono text-2xs cursor-pointer transition-all duration-150",
              categoryFilter === null
                ? "bg-info/15 border-text-accent text-text-accent"
                : "text-text-secondary hover:border-text-accent hover:text-text-primary",
            )}
            onClick={() => setCategoryFilter(null)}
          >
            All
          </button>
          {[...EVENT_CATEGORIES.keys()].map((cat) => (
            <button
              key={cat}
              className={cn(
                "px-2 py-px border border-border-primary rounded-sm bg-bg-primary font-mono text-2xs cursor-pointer transition-all duration-150",
                categoryFilter === cat
                  ? "bg-info/15 border-text-accent text-text-accent"
                  : "text-text-secondary hover:border-text-accent hover:text-text-primary",
              )}
              onClick={() => setCategoryFilter(cat === categoryFilter ? null : cat)}
            >
              {CATEGORY_LABELS.get(cat)}
            </button>
          ))}
        </div>
      </div>

      {/* Event list */}
      <div ref={listRef} className="flex-1 overflow-y-auto">
        {filteredEvents.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
            No events match the current filters
          </div>
        ) : (
          filteredEvents.map((event) => {
            const expanded = expandedIds.has(event.id);
            const cssClass = getEventCategory(event.event_type);
            return (
              <div
                key={event.id}
                className="px-md py-sm border-b border-border-secondary cursor-pointer"
                onClick={() => toggleExpand(event.id)}
              >
                <div className="flex items-start gap-sm">
                  <span className="font-mono text-xs text-text-muted min-w-[48px] shrink-0">
                    {formatTick(event.tick)}
                  </span>
                  <span
                    className={cn(
                      "font-mono text-xs min-w-[100px] shrink-0",
                      eventCategoryColorClass(cssClass),
                    )}
                  >
                    {event.event_type}
                  </span>
                  <span className="text-sm text-text-primary flex-1">
                    {formatEventSummary(event)}
                  </span>
                </div>

                {expanded && (
                  <div className="mt-sm ml-[156px] p-sm bg-bg-primary rounded-sm font-mono text-xs text-text-secondary whitespace-pre-wrap break-all">
                    {JSON.stringify(event.details, null, 2)}
                    {event.world_context && (
                      <div className="mt-xs text-text-muted">
                        World: {event.world_context.era} | {event.world_context.season} |{" "}
                        {event.world_context.weather} | Pop: {event.world_context.population}
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

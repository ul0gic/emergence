/**
 * Timeline Panel (Task 4.5.5 + Phase 9.2)
 *
 * Human-readable event timeline with:
 * - Natural language event narratives (9.2.1)
 * - Severity-based color coding with filter buttons (9.2.2)
 * - Tick grouping with collapsible headers (9.2.3)
 * - Memorial cards for agent deaths (9.2.4)
 *
 * Scrollable, filterable, searchable, auto-scrolling with pause.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { cn } from "../lib/utils.ts";
import type { Event, EventType } from "../types/generated/index.ts";
import { formatNumber, formatTick, getEventCategory } from "../utils/format.ts";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface TimelineProps {
  events: Event[];
  agentNames: Map<string, string>;
  locationNames: Map<string, string>;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Severity system (9.2.2)
// ---------------------------------------------------------------------------

type Severity = "critical" | "notable" | "warning" | "routine" | "info";

const SEVERITY_CONFIG: Record<
  Severity,
  { label: string; dotClass: string; textClass: string; borderClass: string }
> = {
  critical: {
    label: "Critical",
    dotClass: "bg-danger",
    textClass: "text-danger",
    borderClass: "border-l-danger",
  },
  notable: {
    label: "Notable",
    dotClass: "bg-warning",
    textClass: "text-warning",
    borderClass: "border-l-warning",
  },
  warning: {
    label: "Warning",
    dotClass: "bg-[#db6d28]",
    textClass: "text-[#db6d28]",
    borderClass: "border-l-[#db6d28]",
  },
  routine: {
    label: "Routine",
    dotClass: "bg-text-muted",
    textClass: "text-text-muted",
    borderClass: "border-l-text-muted",
  },
  info: {
    label: "Info",
    dotClass: "bg-info",
    textClass: "text-info",
    borderClass: "border-l-info",
  },
};

/** Routine action types that produce low-interest events. */
const ROUTINE_ACTIONS = new Set(["Gather", "Eat", "Drink", "Rest", "NoAction"]);

/**
 * Classify an event into a severity level for coloring and filtering.
 */
function getEventSeverity(event: Event): Severity {
  const details = event.details as Record<string, unknown> | null;

  switch (event.event_type) {
    case "AgentDied":
    case "LedgerAnomaly":
      return "critical";

    case "KnowledgeDiscovered":
    case "AgentBorn":
    case "TradeCompleted":
    case "StructureBuilt":
    case "GroupFormed":
    case "SeasonChanged":
    case "LocationDiscovered":
      return "notable";

    case "ActionRejected":
    case "TradeFailed":
      return "warning";

    case "ActionSucceeded":
    case "ActionSubmitted": {
      const actionType = String(details?.action_type ?? "");
      if (ROUTINE_ACTIONS.has(actionType)) return "routine";
      return "info";
    }

    case "ResourceGathered":
    case "ResourceConsumed":
      return "routine";

    case "TickStart":
    case "TickEnd":
      return "routine";

    default:
      return "info";
  }
}

// ---------------------------------------------------------------------------
// Narrative engine (9.2.1)
// ---------------------------------------------------------------------------

/**
 * Resolve an agent name from ID, bolding it for display.
 * Returns the agent's name wrapped in <strong> tags if found,
 * or the truncated ID otherwise.
 */
function resolveAgent(agentId: string | null | undefined, agentNames: Map<string, string>): string {
  if (!agentId) return "Unknown";
  const name = agentNames.get(agentId);
  return name ? `<strong>${name}</strong>` : agentId.slice(0, 8);
}

/**
 * Resolve a location name from ID, bolding it for display.
 */
function resolveLocation(
  locationId: string | null | undefined,
  locationNames: Map<string, string>,
): string {
  if (!locationId) return "";
  const name = locationNames.get(locationId);
  return name ? `<strong>${name}</strong>` : "";
}

/**
 * Format resource quantities from a Partial<Record<Resource, number>> object
 * into a human-readable string like "3 Wood, 2 FoodBerry".
 */
function formatResourceMap(resources: unknown): string {
  if (!resources || typeof resources !== "object") return "items";
  const entries = Object.entries(resources as Record<string, unknown>).filter(
    ([, v]) => typeof v === "number" && v > 0,
  );
  if (entries.length === 0) return "items";
  return entries.map(([k, v]) => `${v} ${humanizeResourceName(k)}`).join(", ");
}

/**
 * Convert camelCase resource name to space-separated.
 */
function humanizeResourceName(name: string): string {
  return name.replace(/([A-Z])/g, " $1").trim();
}

/**
 * Convert a raw event into a human-readable narrative sentence.
 * Agent names and location names are bolded with <strong> tags.
 */
function formatEventNarrative(
  event: Event,
  agentNames: Map<string, string>,
  locationNames: Map<string, string>,
): string {
  const agent = resolveAgent(event.agent_id, agentNames);
  const loc = resolveLocation(event.location_id, locationNames);
  const atLoc = loc ? ` at ${loc}` : "";
  const details = event.details as Record<string, unknown> | null;

  switch (event.event_type) {
    case "ActionSucceeded":
      return formatActionNarrative(agent, loc, atLoc, details, agentNames, locationNames);

    case "ActionRejected": {
      const actionType = String(details?.action_type ?? "unknown action");
      const reason = String(details?.reason ?? "unknown reason");
      return `${agent} tried to ${actionType.toLowerCase()} but was rejected: ${reason}`;
    }

    case "AgentDied": {
      const cause = String(details?.cause ?? "unknown causes");
      const age = details?.final_age ?? "?";
      return `${agent} died of ${cause} at age ${age}${atLoc}`;
    }

    case "AgentBorn":
      return `A new agent ${agent} was born${atLoc}`;

    case "KnowledgeDiscovered": {
      const knowledge = String(details?.knowledge ?? "something");
      const method = details?.method ? ` via ${details.method}` : "";
      return `${agent} discovered ${knowledge}${method}`;
    }

    case "KnowledgeTaught": {
      const knowledge = String(details?.knowledge ?? "knowledge");
      const student = resolveAgent(details?.student_id as string | undefined, agentNames);
      return `${agent} taught ${knowledge} to ${student}`;
    }

    case "TradeCompleted": {
      const agentB = resolveAgent(details?.agent_b as string | undefined, agentNames);
      const gave = formatResourceMap(details?.gave);
      const received = formatResourceMap(details?.received);
      return `${agent} traded ${gave} for ${received} with ${agentB}`;
    }

    case "TradeFailed": {
      const agentB = resolveAgent(details?.agent_b as string | undefined, agentNames);
      const reason = details?.reason ? `: ${details.reason}` : "";
      return `Trade between ${agent} and ${agentB} failed${reason}`;
    }

    case "StructureBuilt": {
      const structType = humanizeResourceName(String(details?.structure_type ?? "structure"));
      return `${agent} built a ${structType}${atLoc}`;
    }

    case "StructureDestroyed": {
      const structType = humanizeResourceName(String(details?.structure_type ?? "structure"));
      return `${structType} was destroyed${atLoc}`;
    }

    case "StructureRepaired": {
      const structType = humanizeResourceName(String(details?.structure_type ?? "structure"));
      return `${agent} repaired ${structType}${atLoc}`;
    }

    case "RouteImproved": {
      const pathType = String(details?.new_path_type ?? "improved path");
      return `${agent} improved a route to ${humanizeResourceName(pathType)}${atLoc}`;
    }

    case "LocationDiscovered":
      return `${agent} discovered ${loc || "a new location"}`;

    case "ResourceGathered": {
      const qty = details?.quantity ?? "?";
      const resource = humanizeResourceName(String(details?.resource ?? "resources"));
      return `${agent} gathered ${qty} ${resource}${atLoc}`;
    }

    case "ResourceConsumed": {
      const qty = details?.quantity ?? "?";
      const resource = humanizeResourceName(String(details?.resource ?? "resources"));
      return `${agent} consumed ${qty} ${resource}`;
    }

    case "MessageSent": {
      const content = details?.content ? `"${String(details.content).slice(0, 60)}"` : "a message";
      return `${agent} sent ${content}${atLoc}`;
    }

    case "GroupFormed": {
      const groupName = details?.name ? `"${details.name}"` : "a group";
      return `${agent} formed ${groupName}${atLoc}`;
    }

    case "RelationshipChanged": {
      const target = resolveAgent(details?.target as string | undefined, agentNames);
      const delta = details?.delta;
      let direction = "changed";
      if (typeof delta === "number") {
        direction = delta > 0 ? "improved" : "worsened";
      }
      return `${agent}'s relationship with ${target} ${direction}`;
    }

    case "WeatherChanged": {
      const to = details?.to ?? details?.weather ?? "unknown";
      return `Weather changed to ${to}`;
    }

    case "SeasonChanged": {
      const to = details?.to ?? "unknown";
      return `Season changed to ${to}`;
    }

    case "LedgerAnomaly": {
      const description = String(
        details?.description ?? details?.message ?? "Conservation law violated",
      );
      return `Ledger anomaly detected: ${description}`;
    }

    default: {
      const byAgent = agent !== "Unknown" ? ` by ${agent}` : "";
      return `${event.event_type}${byAgent}${atLoc}`;
    }
  }
}

/**
 * Format ActionSucceeded events based on the action_type in details.
 */
function formatActionNarrative(
  agent: string,
  loc: string,
  atLoc: string,
  details: Record<string, unknown> | null,
  agentNames: Map<string, string>,
  locationNames: Map<string, string>,
): string {
  const actionType = String(details?.action_type ?? "");

  switch (actionType) {
    case "Gather": {
      const resource = humanizeResourceName(String(details?.resource ?? "resources"));
      const qty = details?.quantity;
      const qtyStr = qty ? `${qty} ` : "";
      return `${agent} gathered ${qtyStr}${resource}${atLoc}`;
    }

    case "Eat": {
      const resource = humanizeResourceName(String(details?.resource ?? "food"));
      return `${agent} ate ${resource}`;
    }

    case "Drink":
      return `${agent} drank water${atLoc}`;

    case "Rest":
      return `${agent} rested (energy restored)`;

    case "Move": {
      const destId = details?.destination_id as string | undefined;
      const dest = resolveLocation(destId, locationNames);
      return dest ? `${agent} began traveling to ${dest}` : `${agent} began traveling`;
    }

    case "Build": {
      const structType = humanizeResourceName(String(details?.structure_type ?? "structure"));
      return `${agent} built a ${structType}${atLoc}`;
    }

    case "Teach": {
      const knowledge = String(details?.knowledge ?? "knowledge");
      const studentId = details?.target_id as string | undefined;
      const student = resolveAgent(studentId, agentNames);
      return `${agent} taught ${knowledge} to ${student}`;
    }

    case "TradeOffer":
    case "TradeAccept": {
      const targetId = details?.target_id as string | undefined;
      const target = resolveAgent(targetId, agentNames);
      const gave = formatResourceMap(details?.gave ?? details?.offered);
      const received = formatResourceMap(details?.received ?? details?.requested);
      if (gave !== "items" && received !== "items") {
        return `${agent} traded ${gave} for ${received} with ${target}`;
      }
      return `${agent} traded with ${target}`;
    }

    case "Reproduce": {
      const partnerId = details?.partner_id as string | undefined;
      const partner = resolveAgent(partnerId, agentNames);
      return `${agent} and ${partner} had a child`;
    }

    case "NoAction":
      return `${agent} idled`;

    case "Communicate": {
      const targetId = details?.target_id as string | undefined;
      const target = resolveAgent(targetId, agentNames);
      return `${agent} communicated with ${target}${atLoc}`;
    }

    case "Broadcast":
      return `${agent} broadcast a message${atLoc}`;

    case "FormGroup": {
      const groupName = details?.group_name ? `"${details.group_name}"` : "a group";
      return `${agent} formed ${groupName}`;
    }

    case "FarmPlant":
      return `${agent} planted crops${atLoc}`;

    case "FarmHarvest":
      return `${agent} harvested crops${atLoc}`;

    case "Craft": {
      const item = humanizeResourceName(String(details?.item ?? details?.resource ?? "item"));
      return `${agent} crafted ${item}`;
    }

    case "Mine":
      return `${agent} mined resources${atLoc}`;

    case "Smelt":
      return `${agent} smelted ore${atLoc}`;

    case "Write":
      return `${agent} wrote a record${atLoc}`;

    case "Read":
      return `${agent} read a record${atLoc}`;

    case "Repair": {
      const structType = humanizeResourceName(String(details?.structure_type ?? "structure"));
      return `${agent} repaired ${structType}${atLoc}`;
    }

    case "Demolish": {
      const structType = humanizeResourceName(String(details?.structure_type ?? "structure"));
      return `${agent} demolished ${structType}${atLoc}`;
    }

    case "ImproveRoute":
      return `${agent} improved a route${atLoc}`;

    case "Claim":
      return `${agent} claimed territory${atLoc}`;

    case "Legislate": {
      const locSuffix = loc ? ` for ${loc}` : "";
      return `${agent} enacted a law${locSuffix}`;
    }

    case "Enforce":
      return `${agent} enforced a rule${atLoc}`;

    case "TradeReject": {
      const targetId = details?.target_id as string | undefined;
      const target = resolveAgent(targetId, agentNames);
      return `${agent} rejected a trade with ${target}`;
    }

    default: {
      const verb = actionType.toLowerCase();
      return `${agent} performed ${verb}${atLoc}`;
    }
  }
}

// ---------------------------------------------------------------------------
// Tick grouping (9.2.3)
// ---------------------------------------------------------------------------

interface TickGroup {
  tick: number;
  season: string;
  weather: string;
  population: number;
  era: string;
  events: Event[];
}

/**
 * Group a sorted list of events by their tick number.
 * Returns groups in descending tick order (newest first).
 */
function groupEventsByTick(events: Event[]): TickGroup[] {
  const groups = new Map<number, TickGroup>();

  for (const event of events) {
    let group = groups.get(event.tick);
    if (!group) {
      group = {
        tick: event.tick,
        season: event.world_context?.season ?? "?",
        weather: event.world_context?.weather ?? "?",
        population: event.world_context?.population ?? 0,
        era: event.world_context?.era ?? "?",
        events: [],
      };
      groups.set(event.tick, group);
    }
    group.events.push(event);
  }

  // Sort by tick descending (newest first).
  return [...groups.values()].sort((a, b) => b.tick - a.tick);
}

// ---------------------------------------------------------------------------
// Category color helper (retained for expanded details)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/**
 * Memorial card for AgentDied events (9.2.4).
 * Rendered as a prominent, visually distinct card.
 */
function DeathMemorialCard({
  event,
  agentNames,
  locationNames,
  expanded,
  onToggle,
}: {
  event: Event;
  agentNames: Map<string, string>;
  locationNames: Map<string, string>;
  expanded: boolean;
  onToggle: () => void;
}) {
  const details = event.details as Record<string, unknown> | null;
  const agentName = event.agent_id
    ? (agentNames.get(event.agent_id) ?? event.agent_id.slice(0, 8))
    : "Unknown";
  const cause = String(details?.cause ?? "unknown causes");
  const age = String(details?.final_age ?? "?");
  const locName = event.location_id ? (locationNames.get(event.location_id) ?? "") : "";

  return (
    <div
      className="mx-sm my-xs rounded-sm border border-danger/30 bg-danger/5 cursor-pointer overflow-hidden"
      onClick={onToggle}
    >
      <div className="px-md py-sm">
        <div className="flex items-center gap-sm mb-xs">
          <span className="text-danger text-xs font-mono font-semibold uppercase tracking-wide">
            Death
          </span>
        </div>
        <div className="text-base font-semibold text-text-primary">{agentName}</div>
        <div className="text-sm text-text-secondary mt-xs">
          Died at age {age} of {cause}
        </div>
        {locName && <div className="text-xs text-text-muted font-mono mt-xs">{locName}</div>}
      </div>

      {expanded && (
        <div className="px-md pb-sm">
          <div className="p-sm bg-bg-primary rounded-sm font-mono text-xs text-text-secondary whitespace-pre-wrap break-all">
            {JSON.stringify(event.details, null, 2)}
            {event.world_context && (
              <div className="mt-xs text-text-muted">
                World: {event.world_context.era} | {event.world_context.season} |{" "}
                {event.world_context.weather} | Pop: {event.world_context.population}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Regular event row with severity indicator and narrative text.
 */
function EventRow({
  event,
  agentNames,
  locationNames,
  expanded,
  onToggle,
}: {
  event: Event;
  agentNames: Map<string, string>;
  locationNames: Map<string, string>;
  expanded: boolean;
  onToggle: () => void;
}) {
  const severity = getEventSeverity(event);
  // eslint-disable-next-line security/detect-object-injection -- severity is typed as Severity enum, not user input
  const config = SEVERITY_CONFIG[severity];
  const cssClass = getEventCategory(event.event_type);
  const narrative = formatEventNarrative(event, agentNames, locationNames);

  return (
    <div
      className={cn(
        "px-md py-sm border-b border-border-secondary cursor-pointer border-l-2",
        config.borderClass,
      )}
      onClick={onToggle}
    >
      <div className="flex items-start gap-sm">
        <span className={cn("w-1.5 h-1.5 rounded-full mt-1.5 shrink-0", config.dotClass)} />
        <span
          className={cn(
            "font-mono text-2xs min-w-[90px] shrink-0 uppercase",
            eventCategoryColorClass(cssClass),
          )}
        >
          {event.event_type}
        </span>
        <span
          className="text-xs text-text-primary flex-1 [&_strong]:text-text-accent [&_strong]:font-semibold"
          dangerouslySetInnerHTML={{ __html: narrative }}
        />
      </div>

      {expanded && (
        <div className="mt-sm ml-[110px] p-sm bg-bg-primary rounded-sm font-mono text-xs text-text-secondary whitespace-pre-wrap break-all">
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
}

/**
 * Tick group header (9.2.3) -- collapsible card with tick metadata.
 */
function TickGroupHeader({
  group,
  collapsed,
  onToggle,
  eventCount,
}: {
  group: TickGroup;
  collapsed: boolean;
  onToggle: () => void;
  eventCount: number;
}) {
  return (
    <div
      className="flex items-center gap-sm px-md py-xs bg-bg-tertiary border-b border-border-primary cursor-pointer select-none sticky top-0 z-10"
      onClick={onToggle}
    >
      <span className="text-2xs text-text-muted font-mono">{collapsed ? "+" : "-"}</span>
      <span className="font-mono text-xs font-semibold text-text-accent">
        {formatTick(group.tick)}
      </span>
      <span className="text-2xs text-text-secondary font-mono">
        {group.season}, {group.weather}
      </span>
      <span className="text-2xs text-text-muted font-mono">
        {formatNumber(group.population)} alive
      </span>
      <span className="ml-auto text-2xs text-text-muted font-mono">
        {eventCount} event{eventCount !== 1 ? "s" : ""}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function Timeline({ events, agentNames, locationNames }: TimelineProps) {
  const [categoryFilter, setCategoryFilter] = useState<string | null>(null);
  const [searchText, setSearchText] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [collapsedTicks, setCollapsedTicks] = useState<Set<number>>(new Set());
  const [severityFilter, setSeverityFilter] = useState<Set<Severity>>(
    new Set(["critical", "notable", "warning", "info"]),
  );
  const listRef = useRef<HTMLDivElement>(null);

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

    // Severity filter.
    list = list.filter((e) => severityFilter.has(getEventSeverity(e)));

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
  }, [events, categoryFilter, searchText, agentNames, locationNames, severityFilter]);

  // Group by tick.
  const tickGroups = useMemo(() => groupEventsByTick(filteredEvents), [filteredEvents]);

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

  const toggleTickCollapse = useCallback((tick: number) => {
    setCollapsedTicks((prev) => {
      const next = new Set(prev);
      if (next.has(tick)) {
        next.delete(tick);
      } else {
        next.add(tick);
      }
      return next;
    });
  }, []);

  const toggleSeverity = useCallback((severity: Severity) => {
    setSeverityFilter((prev) => {
      const next = new Set(prev);
      if (next.has(severity)) {
        next.delete(severity);
      } else {
        next.add(severity);
      }
      return next;
    });
  }, []);

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      {/* Panel header */}
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

        {/* Severity filter row */}
        <div className="flex gap-xs mb-xs">
          {(Object.keys(SEVERITY_CONFIG) as Severity[]).map((sev) => {
            // eslint-disable-next-line security/detect-object-injection -- sev is typed as Severity enum, not user input
            const cfg = SEVERITY_CONFIG[sev];
            const active = severityFilter.has(sev);
            return (
              <button
                key={sev}
                className={cn(
                  "flex items-center gap-1 px-2 py-px border rounded-sm font-mono text-2xs cursor-pointer transition-all duration-150",
                  active
                    ? cn("border-border-primary bg-bg-elevated", cfg.textClass)
                    : "border-border-secondary bg-bg-primary text-text-muted opacity-50",
                )}
                onClick={() => toggleSeverity(sev)}
              >
                <span className={cn("w-1.5 h-1.5 rounded-full", cfg.dotClass)} />
                {cfg.label}
              </button>
            );
          })}
        </div>

        {/* Category filter row */}
        <div className="flex gap-xs flex-wrap">
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

      {/* Tick-grouped event list */}
      <div ref={listRef} className="flex-1 overflow-y-auto">
        {tickGroups.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
            No events match the current filters
          </div>
        ) : (
          tickGroups.map((group) => {
            const isCollapsed = collapsedTicks.has(group.tick);
            return (
              <div key={group.tick}>
                <TickGroupHeader
                  group={group}
                  collapsed={isCollapsed}
                  onToggle={() => toggleTickCollapse(group.tick)}
                  eventCount={group.events.length}
                />
                {!isCollapsed &&
                  group.events.map((event) => {
                    if (event.event_type === "AgentDied") {
                      return (
                        <DeathMemorialCard
                          key={event.id}
                          event={event}
                          agentNames={agentNames}
                          locationNames={locationNames}
                          expanded={expandedIds.has(event.id)}
                          onToggle={() => toggleExpand(event.id)}
                        />
                      );
                    }
                    return (
                      <EventRow
                        key={event.id}
                        event={event}
                        agentNames={agentNames}
                        locationNames={locationNames}
                        expanded={expandedIds.has(event.id)}
                        onToggle={() => toggleExpand(event.id)}
                      />
                    );
                  })}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

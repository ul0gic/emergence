/**
 * Agent Inspector Panel (Task 4.5.2 + Phase 9.4 Enhancements)
 *
 * List of all agents with search/filter. Click to see deep dive:
 * vitals, personality radar chart, inventory, knowledge, skills,
 * goals, relationships, memory, activity timeline, biography,
 * vitals sparklines, genealogy tree, and comparison mode.
 *
 * Phase 9.4.1: Per-Agent Activity Timeline
 * Phase 9.4.2: Agent Biography Panel
 * Phase 9.4.3: Agent Vitals Sparkline Charts
 * Phase 9.4.4: Agent Genealogy Tree
 * Phase 9.4.5: Agent Comparison Mode
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import * as d3 from "d3";

import { useDebounce } from "../hooks/useApi.ts";
import { cn } from "../lib/utils.ts";
import type {
  Agent,
  AgentDetailResponse,
  AgentId,
  AgentListItem,
  AgentState,
  Event,
  EventType,
  Personality,
  Resource,
} from "../types/generated/index.ts";
import { formatDecimal, formatNumber, formatResourceName } from "../utils/format.ts";

// ---------------------------------------------------------------------------
// Helper utilities
// ---------------------------------------------------------------------------

function getRelationshipColor(score: number): string {
  if (score > 0.5) return "text-positive";
  if (score < -0.2) return "text-negative";
  return "text-neutral";
}

function getMemoryBorderColor(tier: string): string {
  if (tier === "Immediate") return "border-l-danger";
  if (tier === "ShortTerm") return "border-l-warning";
  return "border-l-info";
}

function getVitalFillColor(type: string): string {
  if (type === "energy") return "bg-energy";
  if (type === "health") return "bg-health";
  if (type === "hunger") return "bg-hunger";
  if (type === "thirst") return "bg-info";
  return "bg-hunger";
}

function getVitalStrokeColor(type: string): string {
  if (type === "energy") return "var(--color-energy)";
  if (type === "health") return "var(--color-health)";
  if (type === "hunger") return "var(--color-danger)";
  if (type === "thirst") return "var(--color-info)";
  return "var(--color-text-muted)";
}

function totalInventory(inv: Partial<Record<Resource, number>>): number {
  let total = 0;
  for (const val of Object.values(inv)) {
    total += val ?? 0;
  }
  return total;
}

/** Map event type to a human-readable action description. */
function describeEvent(event: Event, locationNames: Map<string, string>): string {
  const details = event.details as Record<string, unknown> | null;
  const locName = event.location_id ? (locationNames.get(event.location_id) ?? "unknown") : "";

  switch (event.event_type) {
    case "ResourceGathered": {
      const resource = (details?.resource as string) ?? "resource";
      const qty = (details?.quantity as number) ?? 0;
      return `Gathered ${qty} ${formatResourceName(resource as Resource)}`;
    }
    case "ResourceConsumed": {
      const resource = (details?.resource as string) ?? "resource";
      return `Consumed ${formatResourceName(resource as Resource)}`;
    }
    case "ActionSucceeded": {
      const action = (details?.action_type as string) ?? "action";
      return `${action} succeeded`;
    }
    case "ActionRejected": {
      const action = (details?.action_type as string) ?? "action";
      const reason = (details?.reason as string) ?? "";
      return `${action} rejected${reason ? `: ${reason}` : ""}`;
    }
    case "AgentBorn":
      return `Born at ${locName || "unknown location"}`;
    case "AgentDied": {
      const cause = (details?.cause as string) ?? "unknown";
      return `Died: ${cause}`;
    }
    case "TradeCompleted":
      return "Completed a trade";
    case "TradeFailed":
      return "Trade failed";
    case "StructureBuilt": {
      const sType = (details?.structure_type as string) ?? "structure";
      return `Built ${sType}`;
    }
    case "KnowledgeDiscovered": {
      const knowledge = (details?.knowledge as string) ?? "something";
      return `Discovered ${knowledge}`;
    }
    case "KnowledgeTaught": {
      const knowledge = (details?.knowledge as string) ?? "something";
      return `Taught ${knowledge}`;
    }
    case "MessageSent":
      return "Sent a message";
    case "GroupFormed": {
      const groupName = (details?.name as string) ?? "group";
      return `Formed group: ${groupName}`;
    }
    case "RelationshipChanged":
      return "Relationship changed";
    case "LocationDiscovered":
      return `Discovered ${locName || "a new location"}`;
    case "RouteImproved":
      return "Improved a route";
    default:
      return event.event_type;
  }
}

/** Get the CSS class for event type badge. */
function getEventBadgeClass(eventType: EventType): string {
  switch (eventType) {
    case "AgentBorn":
    case "AgentDied":
      return "bg-lifecycle/15 text-lifecycle";
    case "ResourceGathered":
    case "ResourceConsumed":
    case "TradeCompleted":
    case "TradeFailed":
    case "LedgerAnomaly":
      return "bg-economy/15 text-economy";
    case "KnowledgeDiscovered":
    case "KnowledgeTaught":
      return "bg-knowledge/15 text-knowledge";
    case "StructureBuilt":
    case "StructureDestroyed":
    case "StructureRepaired":
    case "RouteImproved":
    case "LocationDiscovered":
      return "bg-world/15 text-world";
    case "MessageSent":
    case "GroupFormed":
    case "RelationshipChanged":
      return "bg-social/15 text-social";
    default:
      return "bg-system/15 text-system";
  }
}

/** Get a trait descriptor based on its value. */
function traitDescriptor(value: number): string {
  if (value >= 0.8) return "very high";
  if (value >= 0.6) return "high";
  if (value >= 0.4) return "moderate";
  if (value >= 0.2) return "low";
  return "very low";
}

/** Format a trait name for prose (e.g., "risk_tolerance" -> "risk tolerance"). */
function formatTraitName(trait: string): string {
  return trait.replace(/_/g, " ");
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface AgentInspectorProps {
  agents: AgentListItem[];
  onSelectAgent: (id: string) => void;
  selectedAgentId: string | null;
  agentDetail: AgentDetailResponse | null;
  events: Event[];
  locationNames: Map<string, string>;
}

type StatusFilter = "all" | "alive" | "dead";

type DetailTab = "overview" | "timeline" | "biography" | "genealogy" | "compare";

/** Vitals history data point for sparkline charts. */
interface VitalsSnapshot {
  tick: number;
  energy: number;
  health: number;
  hunger: number;
  thirst: number;
}

// ---------------------------------------------------------------------------
// Personality Radar Chart (D3)
// ---------------------------------------------------------------------------

function PersonalityRadar({
  personality,
  size = 160,
  className = "",
  overlayPersonality,
  overlayColor,
}: {
  personality: Personality;
  size?: number;
  className?: string;
  overlayPersonality?: Personality;
  overlayColor?: string;
}) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    const center = size / 2;
    const radius = size * 0.34;
    const personalityRecord = personality as unknown as Record<string, string>;
    const traitEntries = Object.entries(personalityRecord);
    const traits = traitEntries.map(([key]) => key);
    const values = traitEntries.map(([, val]) => parseFloat(val ?? "0"));
    const angleSlice = (Math.PI * 2) / traits.length;

    svg.attr("viewBox", `0 0 ${size} ${size}`);

    const g = svg.append("g").attr("transform", `translate(${center},${center})`);

    // Grid circles.
    [0.25, 0.5, 0.75, 1.0].forEach((level) => {
      g.append("circle")
        .attr("r", radius * level)
        .attr("class", "radar-grid");
    });

    // Grid lines.
    traits.forEach((_, i) => {
      const angle = angleSlice * i - Math.PI / 2;
      g.append("line")
        .attr("x1", 0)
        .attr("y1", 0)
        .attr("x2", Math.cos(angle) * radius)
        .attr("y2", Math.sin(angle) * radius)
        .attr("class", "radar-grid");
    });

    // Overlay polygon (comparison agent).
    if (overlayPersonality) {
      const overlayRecord = overlayPersonality as unknown as Record<string, string>;
      const overlayValues = traits.map((t) => parseFloat(overlayRecord[t] ?? "0"));
      const overlayPoints = overlayValues.map((v, i) => {
        const angle = angleSlice * i - Math.PI / 2;
        return `${Math.cos(angle) * radius * v},${Math.sin(angle) * radius * v}`;
      });
      g.append("polygon")
        .attr("points", overlayPoints.join(" "))
        .attr("fill", overlayColor ? `${overlayColor}20` : "rgba(248, 113, 113, 0.15)")
        .attr("stroke", overlayColor ?? "#f87171")
        .attr("stroke-width", 1.5);
    }

    // Data polygon.
    const points = values.map((v, i) => {
      const angle = angleSlice * i - Math.PI / 2;
      return `${Math.cos(angle) * radius * v},${Math.sin(angle) * radius * v}`;
    });

    g.append("polygon").attr("points", points.join(" ")).attr("class", "radar-area");

    // Data dots.
    values.forEach((v, i) => {
      const angle = angleSlice * i - Math.PI / 2;
      g.append("circle")
        .attr("cx", Math.cos(angle) * radius * v)
        .attr("cy", Math.sin(angle) * radius * v)
        .attr("r", 3)
        .attr("class", "radar-dot");
    });

    // Labels.
    const labelAbbr = new Map<string, string>([
      ["curiosity", "CUR"],
      ["cooperation", "COO"],
      ["aggression", "AGG"],
      ["risk_tolerance", "RSK"],
      ["industriousness", "IND"],
      ["sociability", "SOC"],
      ["honesty", "HON"],
      ["loyalty", "LOY"],
    ]);

    traits.forEach((t, i) => {
      const angle = angleSlice * i - Math.PI / 2;
      const lx = Math.cos(angle) * (radius + 16);
      const ly = Math.sin(angle) * (radius + 16);
      g.append("text")
        .attr("x", lx)
        .attr("y", ly)
        .attr("text-anchor", "middle")
        .attr("dominant-baseline", "central")
        .text(labelAbbr.get(t) ?? t.slice(0, 3).toUpperCase());
    });
  }, [personality, size, overlayPersonality, overlayColor]);

  return <svg ref={svgRef} className={cn("radar-chart w-full", className)} />;
}

// ---------------------------------------------------------------------------
// Sparkline Chart (SVG) -- Phase 9.4.3
// ---------------------------------------------------------------------------

function Sparkline({
  data,
  color,
  label,
  currentValue,
  maxValue = 100,
}: {
  data: number[];
  color: string;
  label: string;
  currentValue: number;
  maxValue?: number;
}) {
  const width = 120;
  const height = 28;
  const padding = 2;

  const pathD = useMemo(() => {
    if (data.length < 2) return "";
    const effectiveWidth = width - padding * 2;
    const effectiveHeight = height - padding * 2;
    const step = effectiveWidth / (data.length - 1);

    return data
      .map((val, i) => {
        const x = padding + i * step;
        const y = padding + effectiveHeight - (val / maxValue) * effectiveHeight;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  }, [data, maxValue]);

  const areaD = useMemo(() => {
    if (data.length < 2) return "";
    const effectiveWidth = width - padding * 2;
    const effectiveHeight = height - padding * 2;
    const step = effectiveWidth / (data.length - 1);

    const lineParts = data
      .map((val, i) => {
        const x = padding + i * step;
        const y = padding + effectiveHeight - (val / maxValue) * effectiveHeight;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");

    const lastX = padding + (data.length - 1) * step;
    const bottomY = padding + effectiveHeight;
    return `${lineParts} L${lastX.toFixed(1)},${bottomY} L${padding},${bottomY} Z`;
  }, [data, maxValue]);

  // Determine trend arrow.
  const trend = useMemo(() => {
    if (data.length < 5) return "stable";
    const recent = data.slice(-5);
    const earlier = data.slice(-10, -5);
    if (earlier.length === 0) return "stable";
    const recentAvg = recent.reduce((a, b) => a + b, 0) / recent.length;
    const earlierAvg = earlier.reduce((a, b) => a + b, 0) / earlier.length;
    const diff = recentAvg - earlierAvg;
    if (diff > 3) return "up";
    if (diff < -3) return "down";
    return "stable";
  }, [data]);

  const trendSymbol = trend === "up" ? "^" : trend === "down" ? "v" : "-";
  const trendClass =
    trend === "up" ? "text-success" : trend === "down" ? "text-danger" : "text-text-muted";

  return (
    <div className="flex items-center gap-sm">
      <span className="font-mono text-2xs text-text-secondary w-[52px] text-right">{label}</span>
      <svg width={width} height={height} className="block shrink-0">
        {data.length >= 2 && (
          <>
            <path d={areaD} fill={color} opacity={0.12} />
            <path d={pathD} fill="none" stroke={color} strokeWidth={1.5} />
          </>
        )}
        {data.length < 2 && (
          <text
            x={width / 2}
            y={height / 2}
            textAnchor="middle"
            dominantBaseline="central"
            fill="var(--color-text-muted)"
            fontSize="9"
            fontFamily="var(--font-mono)"
          >
            waiting...
          </text>
        )}
      </svg>
      <span className="font-mono text-2xs text-text-primary w-7 text-right">{currentValue}</span>
      <span className={cn("font-mono text-2xs w-3", trendClass)}>{trendSymbol}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Vital Bar Component
// ---------------------------------------------------------------------------

function VitalBar({
  label,
  value,
  max,
  type,
}: {
  label: string;
  value: number;
  max: number;
  type: string;
}) {
  const pct = Math.min(100, Math.max(0, (value / max) * 100));
  const fillColor = getVitalFillColor(type);

  return (
    <div className="flex items-center gap-sm mb-xs">
      <span className="font-mono text-2xs text-text-secondary w-[52px] text-right">{label}</span>
      <div className="flex-1 h-3 bg-bg-primary rounded-sm overflow-hidden relative">
        <div
          className={cn("h-full rounded-sm transition-[width] duration-300 ease-in-out", fillColor)}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="font-mono text-2xs text-text-primary w-7 text-right">{value}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Section Header Component
// ---------------------------------------------------------------------------

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Per-Agent Activity Timeline -- Phase 9.4.1
// ---------------------------------------------------------------------------

function ActivityTimeline({
  events,
  locationNames,
}: {
  events: Event[];
  locationNames: Map<string, string>;
}) {
  const [displayCount, setDisplayCount] = useState(30);

  const displayedEvents = useMemo(
    () => events.slice(0, displayCount),
    [events, displayCount],
  );

  const hasMore = events.length > displayCount;

  if (events.length === 0) {
    return (
      <div className="text-text-muted font-mono text-xs py-md text-center">
        No activity recorded yet
      </div>
    );
  }

  return (
    <div className="max-h-[400px] overflow-y-auto">
      {displayedEvents.map((event) => (
        <div
          key={event.id}
          className="flex items-start gap-sm py-xs border-b border-border-secondary last:border-b-0"
        >
          <span className="font-mono text-2xs text-text-muted w-14 shrink-0 text-right pt-px">
            T{formatNumber(event.tick)}
          </span>
          <span
            className={cn(
              "inline-flex items-center px-1 rounded-sm text-2xs font-mono shrink-0",
              getEventBadgeClass(event.event_type),
            )}
          >
            {event.event_type.replace(/([A-Z])/g, " $1").trim()}
          </span>
          <span className="text-text-secondary text-2xs">
            {describeEvent(event, locationNames)}
          </span>
        </div>
      ))}
      {hasMore && (
        <button
          className="w-full py-sm text-2xs font-mono text-text-accent bg-transparent border border-border-primary rounded-sm cursor-pointer mt-sm hover:bg-bg-tertiary transition-colors duration-150"
          onClick={() => setDisplayCount((prev) => prev + 30)}
        >
          Load more ({events.length - displayCount} remaining)
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Agent Biography Panel -- Phase 9.4.2
// ---------------------------------------------------------------------------

function AgentBiography({
  agent,
  state,
  agentEvents,
  locationNames,
  agentNameMap,
}: {
  agent: Agent;
  state: AgentState | null;
  agentEvents: Event[];
  locationNames: Map<string, string>;
  agentNameMap: Map<string, string>;
}) {
  const biography = useMemo(() => {
    const parts: string[] = [];
    const isAlive = agent.died_at_tick === null;
    const sex = agent.sex ?? "Male";
    const pronoun = sex === "Female" ? "She" : "He";
    const possessive = sex === "Female" ? "Her" : "His";

    // Opening: name, age, generation, sex.
    if (state) {
      parts.push(
        `${agent.name} is a ${formatNumber(state.age)}-tick-old Generation ${agent.generation} ${sex.toLowerCase()} agent.`,
      );
    } else if (isAlive) {
      parts.push(
        `${agent.name} is a Generation ${agent.generation} ${sex.toLowerCase()} agent.`,
      );
    } else {
      const lifespan =
        agent.died_at_tick !== null
          ? agent.died_at_tick - agent.born_at_tick
          : 0;
      parts.push(
        `${agent.name} was a Generation ${agent.generation} ${sex.toLowerCase()} agent who lived for ${formatNumber(lifespan)} ticks.`,
      );
    }

    // Birth location.
    const bornEvents = agentEvents.filter((e) => e.event_type === "AgentBorn");
    if (bornEvents.length > 0 && bornEvents[0].location_id) {
      const bornLoc = locationNames.get(bornEvents[0].location_id) ?? "an unknown location";
      parts.push(`${pronoun} was born at ${bornLoc}.`);
    }

    // Parents.
    if (agent.parent_a || agent.parent_b) {
      const parentNames: string[] = [];
      if (agent.parent_a) {
        parentNames.push(agentNameMap.get(agent.parent_a) ?? "an unknown agent");
      }
      if (agent.parent_b) {
        parentNames.push(agentNameMap.get(agent.parent_b) ?? "an unknown agent");
      }
      parts.push(`${possessive} parents are ${parentNames.join(" and ")}.`);
    }

    // Current location.
    if (state) {
      const currentLoc = locationNames.get(state.location_id) ?? "an unknown location";
      parts.push(`${pronoun} is currently at ${currentLoc}.`);
    }

    // Activity summary from events.
    const actionCounts = new Map<string, number>();
    for (const event of agentEvents) {
      if (
        event.event_type === "ActionSucceeded" ||
        event.event_type === "ResourceGathered" ||
        event.event_type === "ResourceConsumed"
      ) {
        const key = event.event_type;
        actionCounts.set(key, (actionCounts.get(key) ?? 0) + 1);
      }
    }
    const gatherCount = actionCounts.get("ResourceGathered") ?? 0;
    const consumeCount = actionCounts.get("ResourceConsumed") ?? 0;
    if (gatherCount > 0 || consumeCount > 0) {
      const actionParts: string[] = [];
      if (gatherCount > 0) actionParts.push(`gathered resources ${gatherCount} times`);
      if (consumeCount > 0) actionParts.push(`consumed resources ${consumeCount} times`);
      parts.push(`${pronoun} has ${actionParts.join(" and ")}.`);
    }

    // Discoveries.
    const discoveries = agentEvents.filter((e) => e.event_type === "KnowledgeDiscovered");
    if (discoveries.length > 0) {
      const discoveryNames = discoveries
        .map((e) => {
          const details = e.details as Record<string, unknown> | null;
          return (details?.knowledge as string) ?? "something";
        })
        .slice(0, 5);
      parts.push(
        `${pronoun} has discovered ${discoveryNames.join(", ")}${discoveries.length > 5 ? ` and ${discoveries.length - 5} more` : ""}.`,
      );
    }

    // Personality highlights: top 2 and bottom 2 traits.
    const personalityRecord = agent.personality as unknown as Record<string, string>;
    const traitValues = Object.entries(personalityRecord)
      .map(([k, v]) => ({ name: k, value: parseFloat(v ?? "0") }))
      .sort((a, b) => b.value - a.value);

    if (traitValues.length >= 2) {
      const top = traitValues.slice(0, 2);
      const bottom = traitValues.slice(-2).reverse();
      const topPart = top
        .map((t) => `${traitDescriptor(t.value)} ${formatTraitName(t.name)} (${t.value.toFixed(2)})`)
        .join(" and ");
      const bottomPart = bottom
        .map((t) => `${traitDescriptor(t.value)} ${formatTraitName(t.name)} (${t.value.toFixed(2)})`)
        .join(" and ");
      parts.push(`${possessive} personality is ${topPart}, with ${bottomPart}.`);
    }

    // Current vitals status.
    if (state) {
      const statusParts: string[] = [];
      if (state.hunger > 70) statusParts.push("very hungry");
      else if (state.hunger > 50) statusParts.push("hungry");
      if (state.thirst > 70) statusParts.push("very thirsty");
      else if (state.thirst > 50) statusParts.push("thirsty");
      if (state.energy < 20) statusParts.push("exhausted");
      else if (state.energy < 40) statusParts.push("tired");
      if (state.health < 30) statusParts.push("injured");

      if (statusParts.length > 0) {
        parts.push(`${pronoun} is currently ${statusParts.join(", ")}.`);
      } else {
        parts.push(`${pronoun} is currently in good condition.`);
      }
    }

    // Social status.
    if (state) {
      const relCount = Object.keys(state.relationships).length;
      if (relCount === 0) {
        parts.push(`${pronoun} has no social connections.`);
      } else {
        parts.push(`${pronoun} has ${relCount} social connection${relCount === 1 ? "" : "s"}.`);
      }
    }

    // Cause of death.
    if (!isAlive && agent.cause_of_death) {
      parts.push(`${pronoun} died of ${agent.cause_of_death}.`);
    }

    return parts.join(" ");
  }, [agent, state, agentEvents, locationNames, agentNameMap]);

  return (
    <div className="bg-bg-primary rounded-sm p-md text-sm text-text-secondary leading-relaxed">
      {biography}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Agent Genealogy Tree -- Phase 9.4.4
// ---------------------------------------------------------------------------

interface GenealogyNode {
  id: AgentId;
  name: string;
  generation: number;
  alive: boolean;
  sex: string;
  isCurrentAgent: boolean;
}

function GenealogyTree({
  agent,
  agents,
  onSelectAgent,
}: {
  agent: Agent;
  agents: AgentListItem[];
  onSelectAgent: (id: string) => void;
}) {
  const svgRef = useRef<SVGSVGElement>(null);

  // Build family tree data.
  const familyData = useMemo(() => {
    const agentMap = new Map<string, AgentListItem>();
    for (const a of agents) {
      agentMap.set(a.id, a);
    }

    const nodes: GenealogyNode[] = [];
    const edges: { from: string; to: string; type: "parent" | "child" }[] = [];
    const seen = new Set<string>();

    function addNode(id: string, isCurrentAgent: boolean) {
      if (seen.has(id)) return;
      seen.add(id);
      const agentItem = agentMap.get(id);
      if (agentItem) {
        nodes.push({
          id,
          name: agentItem.name,
          generation: agentItem.generation,
          alive: agentItem.alive,
          sex: "Unknown",
          isCurrentAgent,
        });
      }
    }

    // Add current agent.
    addNode(agent.id, true);
    // Override with real data from full agent detail.
    const currentNode = nodes.find((n) => n.id === agent.id);
    if (currentNode) {
      currentNode.sex = agent.sex ?? "Unknown";
    }

    // Add parents.
    if (agent.parent_a) {
      addNode(agent.parent_a, false);
      edges.push({ from: agent.parent_a, to: agent.id, type: "parent" });
    }
    if (agent.parent_b) {
      addNode(agent.parent_b, false);
      edges.push({ from: agent.parent_b, to: agent.id, type: "parent" });
    }

    // Find children (agents whose parent_a or parent_b is this agent).
    // We can only check agent list items here (they lack parent fields).
    // Use events or rely on the limited data we have. We'll scan all agents
    // checking if any have this agent as a parent. Since AgentListItem doesn't
    // have parent fields, we do a rough approach: any agent born after this
    // agent with generation = agent.generation + 1. But this is unreliable.
    // Instead, we skip children unless we add parent data to list items.
    // For now, rely on the fact that parent_a/parent_b are on the detail.
    // We'll mark that we need the full data for children discovery.

    return { nodes, edges };
  }, [agent, agents]);

  const svgWidth = 400;
  const svgHeight = useMemo(() => {
    const generationSet = new Set(familyData.nodes.map((n) => n.generation));
    return Math.max(120, generationSet.size * 80 + 40);
  }, [familyData]);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();
    svg.attr("viewBox", `0 0 ${svgWidth} ${svgHeight}`);

    if (familyData.nodes.length === 0) return;

    // Group nodes by generation.
    const byGen = new Map<number, GenealogyNode[]>();
    for (const node of familyData.nodes) {
      const gen = node.generation;
      if (!byGen.has(gen)) byGen.set(gen, []);
      byGen.get(gen)!.push(node);
    }

    const sortedGens = Array.from(byGen.keys()).sort((a, b) => a - b);

    // Position nodes.
    const positions = new Map<string, { x: number; y: number }>();
    sortedGens.forEach((gen, genIdx) => {
      const nodesInGen = byGen.get(gen) ?? [];
      const genY = 30 + genIdx * 80;
      const spacing = svgWidth / (nodesInGen.length + 1);
      nodesInGen.forEach((node, nodeIdx) => {
        positions.set(node.id, { x: spacing * (nodeIdx + 1), y: genY });
      });
    });

    const g = svg.append("g");

    // Draw edges.
    for (const edge of familyData.edges) {
      const fromPos = positions.get(edge.from);
      const toPos = positions.get(edge.to);
      if (fromPos && toPos) {
        g.append("line")
          .attr("x1", fromPos.x)
          .attr("y1", fromPos.y + 12)
          .attr("x2", toPos.x)
          .attr("y2", toPos.y - 12)
          .attr("stroke", "var(--color-border-primary)")
          .attr("stroke-width", 1.5)
          .attr("stroke-dasharray", "4,2");
      }
    }

    // Draw nodes.
    for (const node of familyData.nodes) {
      const pos = positions.get(node.id);
      if (!pos) continue;

      const nodeG = g
        .append("g")
        .attr("transform", `translate(${pos.x},${pos.y})`)
        .attr("cursor", "pointer")
        .on("click", () => {
          onSelectAgent(node.id);
        });

      // Node background.
      nodeG
        .append("rect")
        .attr("x", -40)
        .attr("y", -12)
        .attr("width", 80)
        .attr("height", 24)
        .attr("rx", 4)
        .attr("fill", node.isCurrentAgent ? "var(--color-info)" : "var(--color-bg-elevated)")
        .attr("fill-opacity", node.isCurrentAgent ? 0.2 : 1)
        .attr("stroke", node.isCurrentAgent ? "var(--color-info)" : node.alive ? "var(--color-success)" : "var(--color-danger)")
        .attr("stroke-width", node.isCurrentAgent ? 2 : 1);

      // Status dot.
      nodeG
        .append("circle")
        .attr("cx", -32)
        .attr("cy", 0)
        .attr("r", 3)
        .attr("fill", node.alive ? "var(--color-success)" : "var(--color-danger)");

      // Name.
      nodeG
        .append("text")
        .attr("x", 0)
        .attr("y", 1)
        .attr("text-anchor", "middle")
        .attr("dominant-baseline", "central")
        .attr("fill", node.isCurrentAgent ? "var(--color-text-accent)" : "var(--color-text-primary)")
        .attr("font-size", "10px")
        .attr("font-family", "var(--font-mono)")
        .text(node.name.length > 10 ? node.name.slice(0, 9) + "..." : node.name);

      // Generation label below.
      nodeG
        .append("text")
        .attr("x", 0)
        .attr("y", 20)
        .attr("text-anchor", "middle")
        .attr("fill", "var(--color-text-muted)")
        .attr("font-size", "8px")
        .attr("font-family", "var(--font-mono)")
        .text(`Gen ${node.generation}`);
    }
  }, [familyData, svgWidth, svgHeight, onSelectAgent]);

  if (familyData.nodes.length <= 1 && !agent.parent_a && !agent.parent_b) {
    return (
      <div className="text-text-muted font-mono text-xs py-md text-center">
        Generation 0 agent with no recorded lineage
      </div>
    );
  }

  return (
    <svg
      ref={svgRef}
      className="w-full max-w-[400px]"
    />
  );
}

// ---------------------------------------------------------------------------
// Comparison Mode -- Phase 9.4.5
// ---------------------------------------------------------------------------

function ComparisonMode({
  primaryDetail,
  agents,
  agentNameMap: _agentNameMap,
}: {
  primaryDetail: AgentDetailResponse;
  agents: AgentListItem[];
  agentNameMap: Map<string, string>;
}) {
  const [compareAgentId, setCompareAgentId] = useState<string | null>(null);
  const [compareDetail, setCompareDetail] = useState<AgentDetailResponse | null>(null);
  const [compareSearch, setCompareSearch] = useState("");
  const [loading, setLoading] = useState(false);
  const debouncedCompareSearch = useDebounce(compareSearch, 200);

  const filteredCompareAgents = useMemo(() => {
    const q = debouncedCompareSearch.toLowerCase();
    return agents
      .filter((a) => a.id !== primaryDetail.agent.id)
      .filter((a) => !q || a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q))
      .slice(0, 20);
  }, [agents, primaryDetail.agent.id, debouncedCompareSearch]);

  // Fetch comparison agent detail.
  useEffect(() => {
    if (!compareAgentId) {
      setCompareDetail(null);
      return;
    }

    let cancelled = false;
    setLoading(true);

    fetch(`/api/agents/${encodeURIComponent(compareAgentId)}`)
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json() as Promise<unknown>;
      })
      .then((data) => {
        if (!cancelled) {
          setCompareDetail(data as AgentDetailResponse);
          setLoading(false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setCompareDetail(null);
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [compareAgentId]);

  const primary = primaryDetail;
  const compare = compareDetail;

  return (
    <div>
      {/* Agent selector */}
      <div className="mb-md">
        <div className="text-2xs font-mono text-text-muted mb-xs">Select agent to compare with:</div>
        <input
          className="w-full px-md py-sm bg-bg-primary border border-border-primary rounded-sm text-text-primary font-mono text-xs outline-none focus:border-text-accent placeholder:text-text-muted mb-xs"
          placeholder="Search agents..."
          value={compareSearch}
          onChange={(e) => setCompareSearch(e.target.value)}
        />
        {!compareAgentId && (
          <div className="max-h-[120px] overflow-y-auto bg-bg-primary rounded-sm border border-border-primary">
            {filteredCompareAgents.map((a) => (
              <button
                key={a.id}
                className="w-full text-left px-md py-xs text-2xs font-mono text-text-secondary hover:bg-bg-tertiary cursor-pointer bg-transparent border-0 border-b border-border-secondary last:border-b-0 transition-colors duration-100"
                onClick={() => {
                  setCompareAgentId(a.id);
                  setCompareSearch(a.name);
                }}
              >
                {a.name}{" "}
                <span className="text-text-muted">Gen {a.generation}</span>
              </button>
            ))}
            {filteredCompareAgents.length === 0 && (
              <div className="px-md py-xs text-2xs text-text-muted text-center">No agents found</div>
            )}
          </div>
        )}
        {compareAgentId && (
          <button
            className="text-2xs font-mono text-text-accent bg-transparent border border-border-primary rounded-sm px-md py-xs cursor-pointer hover:bg-bg-tertiary transition-colors duration-150"
            onClick={() => {
              setCompareAgentId(null);
              setCompareSearch("");
              setCompareDetail(null);
            }}
          >
            Clear selection
          </button>
        )}
      </div>

      {loading && (
        <div className="text-text-muted font-mono text-xs py-md text-center">
          Loading comparison...
        </div>
      )}

      {compare && !loading && (
        <div className="space-y-md">
          {/* Header comparison */}
          <div className="grid grid-cols-2 gap-md">
            <div className="bg-bg-primary rounded-sm p-sm">
              <div className="text-sm font-semibold text-text-accent font-mono">{primary.agent.name}</div>
              <div className="text-2xs text-text-muted font-mono">
                Gen {primary.agent.generation} | {primary.agent.sex}
              </div>
            </div>
            <div className="bg-bg-primary rounded-sm p-sm">
              <div className="text-sm font-semibold text-chart-5 font-mono">{compare.agent.name}</div>
              <div className="text-2xs text-text-muted font-mono">
                Gen {compare.agent.generation} | {compare.agent.sex}
              </div>
            </div>
          </div>

          {/* Personality radar overlay */}
          <SectionHeader>Personality Comparison</SectionHeader>
          <div className="flex justify-center">
            <PersonalityRadar
              personality={primary.agent.personality}
              overlayPersonality={compare.agent.personality}
              overlayColor="var(--color-chart-5)"
              size={240}
              className="max-w-[280px]"
            />
          </div>
          <div className="flex justify-center gap-lg text-2xs font-mono">
            <span className="text-text-accent">-- {primary.agent.name}</span>
            <span className="text-chart-5">-- {compare.agent.name}</span>
          </div>

          {/* Vitals comparison bars */}
          {primary.state && compare.state && (
            <>
              <SectionHeader>Vitals Comparison</SectionHeader>
              <ComparisonBar
                label="Energy"
                valueA={primary.state.energy}
                valueB={compare.state.energy}
                max={100}
                nameA={primary.agent.name}
                nameB={compare.agent.name}
              />
              <ComparisonBar
                label="Health"
                valueA={primary.state.health}
                valueB={compare.state.health}
                max={100}
                nameA={primary.agent.name}
                nameB={compare.agent.name}
              />
              <ComparisonBar
                label="Hunger"
                valueA={primary.state.hunger}
                valueB={compare.state.hunger}
                max={100}
                nameA={primary.agent.name}
                nameB={compare.agent.name}
              />
              <ComparisonBar
                label="Thirst"
                valueA={primary.state.thirst}
                valueB={compare.state.thirst}
                max={100}
                nameA={primary.agent.name}
                nameB={compare.agent.name}
              />
            </>
          )}

          {/* Inventory comparison */}
          {primary.state && compare.state && (
            <>
              <SectionHeader>Inventory Comparison</SectionHeader>
              <InventoryComparison
                invA={primary.state.inventory}
                invB={compare.state.inventory}
                nameA={primary.agent.name}
                nameB={compare.agent.name}
              />
            </>
          )}

          {/* Knowledge diff */}
          {primary.state && compare.state && (
            <>
              <SectionHeader>Knowledge Comparison</SectionHeader>
              <KnowledgeDiff
                knowledgeA={primary.state.knowledge}
                knowledgeB={compare.state.knowledge}
                nameA={primary.agent.name}
                nameB={compare.agent.name}
              />
            </>
          )}

          {/* Skills comparison */}
          {primary.state && compare.state && (
            <>
              <SectionHeader>Skills Comparison</SectionHeader>
              <SkillsComparison
                skillsA={primary.state.skills}
                skillsB={compare.state.skills}
                nameA={primary.agent.name}
                nameB={compare.agent.name}
              />
            </>
          )}
        </div>
      )}

      {!compare && !loading && compareAgentId && (
        <div className="text-text-muted font-mono text-xs py-md text-center">
          Could not load agent data
        </div>
      )}
    </div>
  );
}

function ComparisonBar({
  label,
  valueA,
  valueB,
  max,
}: {
  label: string;
  valueA: number;
  valueB: number;
  max: number;
  nameA: string;
  nameB: string;
}) {
  const pctA = Math.min(100, Math.max(0, (valueA / max) * 100));
  const pctB = Math.min(100, Math.max(0, (valueB / max) * 100));

  return (
    <div className="mb-sm">
      <div className="flex justify-between text-2xs font-mono text-text-secondary mb-xs">
        <span>{label}</span>
        <span>{valueA} vs {valueB}</span>
      </div>
      <div className="flex gap-xs">
        <div className="flex-1 h-2 bg-bg-primary rounded-sm overflow-hidden">
          <div
            className="h-full bg-text-accent rounded-sm transition-[width] duration-300"
            style={{ width: `${pctA}%` }}
          />
        </div>
        <div className="flex-1 h-2 bg-bg-primary rounded-sm overflow-hidden">
          <div
            className="h-full bg-chart-5 rounded-sm transition-[width] duration-300"
            style={{ width: `${pctB}%` }}
          />
        </div>
      </div>
    </div>
  );
}

function InventoryComparison({
  invA,
  invB,
  nameA,
  nameB,
}: {
  invA: Partial<Record<Resource, number>>;
  invB: Partial<Record<Resource, number>>;
  nameA: string;
  nameB: string;
}) {
  const allResources = useMemo(() => {
    const set = new Set<Resource>();
    for (const r of Object.keys(invA) as Resource[]) {
      if ((invA[r] ?? 0) > 0) set.add(r);
    }
    for (const r of Object.keys(invB) as Resource[]) {
      if ((invB[r] ?? 0) > 0) set.add(r);
    }
    return Array.from(set).sort();
  }, [invA, invB]);

  if (allResources.length === 0) {
    return <div className="text-text-muted text-xs">Both inventories are empty</div>;
  }

  return (
    <div className="grid grid-cols-3 gap-xs text-2xs font-mono">
      <div className="text-text-accent">{nameA}</div>
      <div className="text-center text-text-muted">Resource</div>
      <div className="text-right text-chart-5">{nameB}</div>
      {allResources.map((r) => (
        <div key={r} className="contents">
          <div className="text-text-primary">{invA[r] ?? 0}</div>
          <div className="text-center text-text-secondary">{formatResourceName(r)}</div>
          <div className="text-right text-text-primary">{invB[r] ?? 0}</div>
        </div>
      ))}
    </div>
  );
}

function KnowledgeDiff({
  knowledgeA,
  knowledgeB,
  nameA,
  nameB,
}: {
  knowledgeA: string[];
  knowledgeB: string[];
  nameA: string;
  nameB: string;
}) {
  const setA = useMemo(() => new Set(knowledgeA), [knowledgeA]);
  const setB = useMemo(() => new Set(knowledgeB), [knowledgeB]);

  const shared = useMemo(
    () => knowledgeA.filter((k) => setB.has(k)),
    [knowledgeA, setB],
  );
  const onlyA = useMemo(
    () => knowledgeA.filter((k) => !setB.has(k)),
    [knowledgeA, setB],
  );
  const onlyB = useMemo(
    () => knowledgeB.filter((k) => !setA.has(k)),
    [knowledgeB, setA],
  );

  return (
    <div className="space-y-xs">
      {shared.length > 0 && (
        <div>
          <div className="text-2xs font-mono text-text-muted mb-xs">Shared ({shared.length})</div>
          <div className="flex flex-wrap gap-xs">
            {shared.map((k) => (
              <span
                key={k}
                className="px-1.5 py-px bg-success/10 border border-success/30 rounded-sm font-mono text-2xs text-success"
              >
                {k}
              </span>
            ))}
          </div>
        </div>
      )}
      {onlyA.length > 0 && (
        <div>
          <div className="text-2xs font-mono text-text-accent mb-xs">
            Only {nameA} ({onlyA.length})
          </div>
          <div className="flex flex-wrap gap-xs">
            {onlyA.map((k) => (
              <span
                key={k}
                className="px-1.5 py-px bg-info/10 border border-info/30 rounded-sm font-mono text-2xs text-info"
              >
                {k}
              </span>
            ))}
          </div>
        </div>
      )}
      {onlyB.length > 0 && (
        <div>
          <div className="text-2xs font-mono text-chart-5 mb-xs">
            Only {nameB} ({onlyB.length})
          </div>
          <div className="flex flex-wrap gap-xs">
            {onlyB.map((k) => (
              <span
                key={k}
                className="px-1.5 py-px bg-chart-5/10 border border-chart-5/30 rounded-sm font-mono text-2xs text-chart-5"
              >
                {k}
              </span>
            ))}
          </div>
        </div>
      )}
      {shared.length === 0 && onlyA.length === 0 && onlyB.length === 0 && (
        <div className="text-text-muted text-xs">Neither agent has any knowledge</div>
      )}
    </div>
  );
}

function SkillsComparison({
  skillsA,
  skillsB,
  nameA: _nameA,
  nameB: _nameB,
}: {
  skillsA: Record<string, number | undefined>;
  skillsB: Record<string, number | undefined>;
  nameA: string;
  nameB: string;
}) {
  const allSkills = useMemo(() => {
    const set = new Set<string>();
    for (const [k, v] of Object.entries(skillsA)) {
      if ((v ?? 0) > 0) set.add(k);
    }
    for (const [k, v] of Object.entries(skillsB)) {
      if ((v ?? 0) > 0) set.add(k);
    }
    return Array.from(set).sort();
  }, [skillsA, skillsB]);

  if (allSkills.length === 0) {
    return <div className="text-text-muted text-xs">Neither agent has skills</div>;
  }

  return (
    <div className="space-y-xs">
      {allSkills.map((skill) => {
        const valA = skillsA[skill] ?? 0;
        const valB = skillsB[skill] ?? 0;
        const maxVal = Math.max(valA, valB, 1);
        return (
          <div key={skill} className="mb-xs">
            <div className="flex justify-between text-2xs font-mono text-text-secondary mb-px">
              <span>{skill}</span>
              <span>{valA} vs {valB}</span>
            </div>
            <div className="flex gap-xs">
              <div className="flex-1 h-2 bg-bg-primary rounded-sm overflow-hidden">
                <div
                  className="h-full bg-text-accent rounded-sm"
                  style={{ width: `${(valA / maxVal) * 100}%` }}
                />
              </div>
              <div className="flex-1 h-2 bg-bg-primary rounded-sm overflow-hidden">
                <div
                  className="h-full bg-chart-5 rounded-sm"
                  style={{ width: `${(valB / maxVal) * 100}%` }}
                />
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Agent Inspector Component (Main)
// ---------------------------------------------------------------------------

export default function AgentInspector({
  agents,
  onSelectAgent,
  selectedAgentId,
  agentDetail,
  events,
  locationNames,
}: AgentInspectorProps) {
  const [searchText, setSearchText] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const debouncedSearch = useDebounce(searchText, 200);

  const filteredAgents = useMemo(() => {
    let list = agents;
    if (statusFilter === "alive") list = list.filter((a) => a.alive);
    if (statusFilter === "dead") list = list.filter((a) => !a.alive);
    if (debouncedSearch) {
      const q = debouncedSearch.toLowerCase();
      list = list.filter((a) => a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q));
    }
    return list;
  }, [agents, statusFilter, debouncedSearch]);

  const handleSelect = useCallback(
    (id: string) => {
      onSelectAgent(id);
    },
    [onSelectAgent],
  );

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        Agent Inspector
      </div>
      <div className="flex flex-1 overflow-hidden">
        {/* Left: Agent list */}
        <div className="w-60 border-r border-border-primary flex flex-col">
          <div className="p-sm">
            <input
              className="w-full px-md py-sm bg-bg-primary border border-border-primary rounded-sm text-text-primary font-mono text-xs outline-none focus:border-text-accent placeholder:text-text-muted"
              placeholder="Search agents..."
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
            />
            <div className="flex gap-xs mt-xs">
              {(["all", "alive", "dead"] as const).map((f) => (
                <button
                  key={f}
                  className={cn(
                    "px-2 py-px border border-border-primary rounded-sm bg-bg-primary font-mono text-2xs cursor-pointer transition-all duration-150",
                    statusFilter === f
                      ? "bg-info/15 border-text-accent text-text-accent"
                      : "text-text-secondary hover:border-text-accent hover:text-text-primary",
                  )}
                  onClick={() => setStatusFilter(f)}
                >
                  {f}
                </button>
              ))}
            </div>
          </div>
          <ul className="list-none overflow-y-auto flex-1">
            {filteredAgents.map((agent) => (
              <li
                key={agent.id}
                className={cn(
                  "flex items-center justify-between px-md py-sm border-b border-border-secondary cursor-pointer transition-colors duration-100 text-xs hover:bg-bg-tertiary",
                  selectedAgentId === agent.id && "bg-bg-elevated border-l-2 border-l-text-accent",
                )}
                onClick={() => handleSelect(agent.id)}
              >
                <div>
                  <div className="font-semibold text-sm">{agent.name}</div>
                  <div className="text-2xs text-text-muted font-mono">
                    Gen {agent.generation}
                    {agent.vitals ? ` | Age ${agent.vitals.age}` : ""}
                  </div>
                </div>
                <span
                  className={cn(
                    "inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold",
                    agent.alive ? "bg-success/15 text-success" : "bg-danger/15 text-danger",
                  )}
                >
                  {agent.alive ? "alive" : "dead"}
                </span>
              </li>
            ))}
            {filteredAgents.length === 0 && (
              <li className="flex items-center justify-center px-md py-sm text-xs text-text-muted cursor-default">
                No agents found
              </li>
            )}
          </ul>
        </div>

        {/* Right: Agent detail */}
        <div className="flex-1 overflow-y-auto p-md">
          {agentDetail ? (
            <AgentDetail
              detail={agentDetail}
              agents={agents}
              events={events}
              locationNames={locationNames}
              onSelectAgent={onSelectAgent}
            />
          ) : (
            <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
              Select an agent to inspect
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Agent Detail View (Enhanced)
// ---------------------------------------------------------------------------

function AgentDetail({
  detail,
  agents,
  events,
  locationNames,
  onSelectAgent,
}: {
  detail: AgentDetailResponse;
  agents: AgentListItem[];
  events: Event[];
  locationNames: Map<string, string>;
  onSelectAgent: (id: string) => void;
}) {
  const { agent, state } = detail;
  const [activeTab, setActiveTab] = useState<DetailTab>("overview");

  // Vitals history for sparklines (Phase 9.4.3).
  // Collected from events that have agent_state_snapshot for this agent.
  const vitalsHistory = useMemo<VitalsSnapshot[]>(() => {
    const agentEvents = events
      .filter((e) => e.agent_id === agent.id && e.agent_state_snapshot !== null)
      .sort((a, b) => a.tick - b.tick);

    const snapshots: VitalsSnapshot[] = [];
    const seenTicks = new Set<number>();

    for (const event of agentEvents) {
      if (seenTicks.has(event.tick)) continue;
      seenTicks.add(event.tick);
      const snap = event.agent_state_snapshot;
      if (snap) {
        snapshots.push({
          tick: event.tick,
          energy: snap.energy,
          health: snap.health,
          hunger: snap.hunger,
          thirst: 0, // AgentStateSnapshot doesn't include thirst; use 0 as fallback.
        });
      }
    }

    // Keep last 50 data points.
    return snapshots.slice(-50);
  }, [events, agent.id]);

  const agentNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agents) {
      map.set(a.id, a.name);
    }
    return map;
  }, [agents]);

  // Filter events for this agent (Phase 9.4.1).
  const agentEvents = useMemo(() => {
    return events
      .filter((e) => e.agent_id === agent.id)
      .filter(
        (e) =>
          e.event_type !== "TickStart" &&
          e.event_type !== "TickEnd",
      )
      .sort((a, b) => b.tick - a.tick);
  }, [events, agent.id]);

  const detailTabs: { id: DetailTab; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "timeline", label: "Timeline" },
    { id: "biography", label: "Biography" },
    { id: "genealogy", label: "Genealogy" },
    { id: "compare", label: "Compare" },
  ];

  return (
    <div>
      {/* Header */}
      <div className="flex items-center gap-md mb-sm">
        <div>
          <h2 className="text-lg text-text-accent font-mono m-0">{agent.name}</h2>
          <div className="text-xs text-text-muted font-mono">
            Gen {agent.generation} | {agent.sex} | Born T{formatNumber(agent.born_at_tick)}
            {agent.died_at_tick !== null ? ` | Died T${formatNumber(agent.died_at_tick)}` : ""}
          </div>
        </div>
        <span
          className={cn(
            "inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold",
            agent.died_at_tick === null ? "bg-success/15 text-success" : "bg-danger/15 text-danger",
          )}
        >
          {agent.died_at_tick === null ? "alive" : (agent.cause_of_death ?? "dead")}
        </span>
      </div>

      {/* Detail tab bar */}
      <nav className="flex border-b border-border-primary mb-md">
        {detailTabs.map((tab) => (
          <button
            key={tab.id}
            className={cn(
              "px-md py-xs text-2xs font-mono bg-transparent border-0 border-b-2 border-b-transparent cursor-pointer whitespace-nowrap transition-colors duration-150",
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

      {/* Tab content */}
      {activeTab === "overview" && (
        <OverviewTab
          agent={agent}
          state={state}
          agentNameMap={agentNameMap}
          vitalsHistory={vitalsHistory}
        />
      )}

      {activeTab === "timeline" && (
        <>
          <SectionHeader>
            Activity Timeline ({agentEvents.length} events)
          </SectionHeader>
          <ActivityTimeline events={agentEvents} locationNames={locationNames} />
        </>
      )}

      {activeTab === "biography" && (
        <>
          <SectionHeader>Biography</SectionHeader>
          <AgentBiography
            agent={agent}
            state={state}
            agentEvents={agentEvents}
            locationNames={locationNames}
            agentNameMap={agentNameMap}
          />
        </>
      )}

      {activeTab === "genealogy" && (
        <>
          <SectionHeader>Genealogy</SectionHeader>
          <GenealogyTree agent={agent} agents={agents} onSelectAgent={onSelectAgent} />
        </>
      )}

      {activeTab === "compare" && (
        <>
          <SectionHeader>Agent Comparison</SectionHeader>
          <ComparisonMode
            primaryDetail={detail}
            agents={agents}
            agentNameMap={agentNameMap}
          />
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Overview Tab (Original detail view, enhanced with sparklines)
// ---------------------------------------------------------------------------

function OverviewTab({
  agent,
  state,
  agentNameMap,
  vitalsHistory,
}: {
  agent: Agent;
  state: AgentState | null;
  agentNameMap: Map<string, string>;
  vitalsHistory: VitalsSnapshot[];
}) {
  return (
    <>
      {state && (
        <>
          {/* Vitals with Sparklines (Phase 9.4.3) */}
          <SectionHeader>Vitals</SectionHeader>
          <div className="mb-md">
            {/* Current bars */}
            <VitalBar label="Energy" value={state.energy} max={100} type="energy" />
            <VitalBar label="Health" value={state.health} max={100} type="health" />
            <VitalBar label="Hunger" value={state.hunger} max={100} type="hunger" />
            <VitalBar label="Thirst" value={state.thirst} max={100} type="thirst" />
            <div className="flex gap-lg mt-sm text-xs font-mono text-text-secondary">
              <span>Age: {formatNumber(state.age)} ticks</span>
              <span>
                Carry: {totalInventory(state.inventory)}/{state.carry_capacity}
              </span>
            </div>
          </div>

          {/* Sparkline trends */}
          {vitalsHistory.length > 1 && (
            <>
              <SectionHeader>Vitals Trend (Last {vitalsHistory.length} snapshots)</SectionHeader>
              <div className="mb-md space-y-xs">
                <Sparkline
                  data={vitalsHistory.map((v) => v.energy)}
                  color={getVitalStrokeColor("energy")}
                  label="Energy"
                  currentValue={state.energy}
                />
                <Sparkline
                  data={vitalsHistory.map((v) => v.health)}
                  color={getVitalStrokeColor("health")}
                  label="Health"
                  currentValue={state.health}
                />
                <Sparkline
                  data={vitalsHistory.map((v) => v.hunger)}
                  color={getVitalStrokeColor("hunger")}
                  label="Hunger"
                  currentValue={state.hunger}
                />
              </div>
            </>
          )}

          {/* Personality + Inventory side by side */}
          <div className="flex gap-lg">
            <div>
              <SectionHeader>Personality</SectionHeader>
              <PersonalityRadar personality={agent.personality} className="max-w-[180px]" />
            </div>
            <div className="flex-1">
              <SectionHeader>Inventory</SectionHeader>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-xs">
                {Object.entries(state.inventory)
                  .filter(([, qty]) => qty !== undefined && qty > 0)
                  .map(([resource, qty]) => (
                    <div
                      key={resource}
                      className="flex justify-between items-center px-sm py-xs bg-bg-primary rounded-sm font-mono text-2xs"
                    >
                      <span className="text-text-secondary">
                        {formatResourceName(resource as Resource)}
                      </span>
                      <span className="text-text-primary font-semibold">{qty}</span>
                    </div>
                  ))}
                {Object.values(state.inventory).every((v) => !v || v === 0) && (
                  <span className="text-text-muted text-xs">Empty</span>
                )}
              </div>
            </div>
          </div>

          {/* Goals */}
          <SectionHeader>Goals</SectionHeader>
          {state.goals.length > 0 ? (
            <ul className="pl-lg text-text-secondary text-sm">
              {state.goals.map((goal, i) => (
                <li key={i} className="mb-xs">
                  {goal}
                </li>
              ))}
            </ul>
          ) : (
            <span className="text-text-muted text-xs">No active goals</span>
          )}

          {/* Skills */}
          <SectionHeader>Skills</SectionHeader>
          {Object.entries(state.skills)
            .filter(([, level]) => level !== undefined && level > 0)
            .map(([name, level]) => (
              <div key={name} className="flex items-center gap-sm mb-xs">
                <span className="font-mono text-2xs text-text-secondary w-20">{name}</span>
                <div className="flex-1 h-2 bg-bg-primary rounded-sm overflow-hidden">
                  <div
                    className="h-full bg-chart-4 rounded-sm"
                    style={{ width: `${Math.min((level ?? 0) * 10, 100)}%` }}
                  />
                </div>
                <span className="font-mono text-2xs text-text-primary w-5 text-right">{level}</span>
              </div>
            ))}

          {/* Knowledge */}
          <SectionHeader>Knowledge ({state.knowledge.length})</SectionHeader>
          <div className="flex flex-wrap gap-xs">
            {state.knowledge.map((k) => (
              <span
                key={k}
                className="px-1.5 py-px bg-success/10 border border-success/30 rounded-sm font-mono text-2xs text-success"
              >
                {k}
              </span>
            ))}
          </div>

          {/* Relationships */}
          <SectionHeader>Relationships</SectionHeader>
          {Object.entries(state.relationships).length > 0 ? (
            <div className="text-sm">
              {Object.entries(state.relationships)
                .filter(([, score]) => score !== undefined)
                .map(([agentId, score]) => {
                  const scoreNum = parseFloat(score ?? "0");
                  const colorClass = getRelationshipColor(scoreNum);
                  return (
                    <div
                      key={agentId}
                      className="flex justify-between py-xs border-b border-border-secondary"
                    >
                      <span>{agentNameMap.get(agentId) ?? agentId.slice(0, 8)}</span>
                      <span className={cn("font-mono", colorClass)}>
                        {formatDecimal(score ?? "0", 2)}
                      </span>
                    </div>
                  );
                })}
            </div>
          ) : (
            <span className="text-text-muted text-xs">No relationships</span>
          )}

          {/* Memory */}
          <SectionHeader>Memory ({state.memory.length})</SectionHeader>
          <div className="max-h-[200px] overflow-y-auto">
            {state.memory.map((mem, i) => {
              const borderColor = getMemoryBorderColor(mem.tier);
              return (
                <div key={i} className={cn("px-sm py-xs border-l-2 mb-xs text-2xs", borderColor)}>
                  <span className="text-text-muted font-mono">T{mem.tick} </span>
                  <span className="text-text-secondary">{mem.summary}</span>
                </div>
              );
            })}
            {state.memory.length === 0 && (
              <span className="text-text-muted text-xs">No memories</span>
            )}
          </div>
        </>
      )}
    </>
  );
}

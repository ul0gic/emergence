/**
 * Discovery Log and Era Tracker (Task 4.5.7)
 *
 * Chronological list of discoveries. Current era display. Tech tree
 * progress visualization. Knowledge distribution across agents.
 */
import { useMemo } from "react";

import type { AgentListItem, Era, Event, WorldSnapshot } from "../types/generated/index.ts";
import { formatNumber, formatTick } from "../utils/format.ts";

interface DiscoveryLogProps {
  worldSnapshot: WorldSnapshot | null;
  events: Event[];
  agents: AgentListItem[];
  agentNames: Map<string, string>;
}

const ERA_ORDER: Era[] = [
  "Primitive",
  "Tribal",
  "Agricultural",
  "Settlement",
  "Bronze",
  "Iron",
  "Classical",
  "Medieval",
  "Industrial",
  "Modern",
];

const ERA_TRIGGERS: Record<Era, string> = {
  Primitive: "Starting era",
  Tribal: "Group formation emerged",
  Agricultural: "Farming discovered",
  Settlement: "Permanent structures established",
  Bronze: "Metalworking discovered",
  Iron: "Advanced metalworking",
  Classical: "Written language and governance",
  Medieval: "Complex institutions",
  Industrial: "Manufacturing",
  Modern: "Full technology",
};

function getTechNodeBorderColor(discovered: boolean, adjacent: boolean): string {
  if (discovered) return "var(--color-success)";
  if (adjacent) return "var(--color-warning)";
  return "var(--color-border-primary)";
}

function getTechNodeBackground(discovered: boolean, adjacent: boolean): string {
  if (discovered) return "rgba(63, 185, 80, 0.1)";
  if (adjacent) return "rgba(210, 153, 34, 0.1)";
  return "var(--color-bg-primary)";
}

function getTechNodeColor(discovered: boolean, adjacent: boolean): string {
  if (discovered) return "var(--color-success)";
  if (adjacent) return "var(--color-warning)";
  return "var(--color-text-muted)";
}

function getTechNodeTitle(node: {
  discovered: boolean;
  adjacent: boolean;
  prerequisites: string[];
}): string {
  if (node.discovered) return "Discovered";
  if (node.adjacent) return `Adjacent (needs: ${node.prerequisites.join(", ")})`;
  return "Undiscovered";
}

function getTechNodeLabel(node: { discovered: boolean; adjacent: boolean; name: string }): string {
  if (node.discovered || node.adjacent) return node.name;
  return "???";
}

function getEraOpacity(i: number, currentEraIndex: number): number {
  if (i === currentEraIndex) return 1;
  if (i < currentEraIndex) return 0.5;
  return 0.2;
}

/** The discovery adjacency map (tech tree). */
const TECH_TREE: Record<string, string[]> = {
  cooking: ["gather_food", "build_campfire"],
  agriculture: ["gather_food", "observe_seasons"],
  food_preservation: ["agriculture", "build_storage"],
  basic_tools: ["gather_wood", "gather_stone"],
  mining: ["basic_tools", "gather_stone"],
  smelting: ["mining", "build_campfire"],
  metalworking: ["smelting", "basic_tools"],
  oral_tradition: ["basic_communication", "group_formation"],
  written_language: ["oral_tradition", "clay"],
  library: ["written_language", "build_hut"],
  barter_system: ["basic_trade", "group_formation"],
  currency_concept: ["barter_system", "written_language"],
  taxation: ["currency_concept", "group_formation"],
  governance: ["group_formation", "territorial_claim"],
  legislation: ["governance", "written_language"],
  justice_system: ["legislation", "group_formation"],
};

export default function DiscoveryLog({
  worldSnapshot: snapshot,
  events,
  agents: _agents,
  agentNames,
}: DiscoveryLogProps) {
  // _agents available for future per-agent knowledge breakdown.

  const discoveries = useMemo(() => snapshot?.discoveries ?? [], [snapshot]);
  const currentEra = snapshot?.era ?? "Primitive";
  const currentEraIndex = ERA_ORDER.indexOf(currentEra);

  // Extract discovery events from the event log.
  const discoveryEvents = useMemo(() => {
    return events
      .filter((e) => e.event_type === "KnowledgeDiscovered")
      .map((e) => {
        const details = e.details as Record<string, unknown>;
        return {
          tick: e.tick,
          agentId: e.agent_id ?? "",
          agentName: e.agent_id
            ? (agentNames.get(e.agent_id) ?? e.agent_id.slice(0, 8))
            : "Unknown",
          knowledge: (details?.knowledge as string) ?? "unknown",
          method: (details?.method as string) ?? "unknown",
          prerequisites: (details?.prerequisites as string[]) ?? [],
        };
      })
      .sort((a, b) => b.tick - a.tick);
  }, [events, agentNames]);

  // Tech tree nodes: what is discovered and what is adjacent.
  const techTreeStatus = useMemo(() => {
    const discoveredSet = new Set(discoveries);
    const nodes: {
      name: string;
      discovered: boolean;
      adjacent: boolean;
      prerequisites: string[];
    }[] = [];

    for (const [node, prereqs] of Object.entries(TECH_TREE)) {
      const discovered = discoveredSet.has(node);
      const adjacent = !discovered && prereqs.every((p) => discoveredSet.has(p));
      nodes.push({
        name: node,
        discovered,
        adjacent,
        prerequisites: prereqs,
      });
    }

    return nodes;
  }, [discoveries]);

  if (!snapshot) {
    return (
      <div className="h-full bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
        <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
          Discovery Log
        </div>
        <div className="p-md">
          <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
            Waiting for world data...
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>Discovery Log & Era Tracker</span>
        <span className="text-xs font-normal">{formatNumber(discoveries.length)} discoveries</span>
      </div>
      <div className="p-md flex-1 overflow-y-auto">
        {/* Current Era Display */}
        <div className="flex items-center gap-md p-md bg-bg-tertiary rounded-md mb-md">
          <div>
            <div className="text-2xs text-text-secondary uppercase">Current Era</div>
            <div className="text-xl font-bold text-lifecycle font-mono">{currentEra}</div>
          </div>
          <div className="flex-1 text-xs text-text-secondary">
            {/* eslint-disable-next-line security/detect-object-injection -- currentEra is typed as Era from WorldSnapshot, not user input */}
            {ERA_TRIGGERS[currentEra]}
          </div>
        </div>

        {/* Era Progress Bar */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Era Progress
        </div>
        <div className="flex gap-0.5 mb-md">
          {ERA_ORDER.map((era, i) => (
            <div
              key={era}
              className="flex-1 h-1.5 rounded-sm"
              style={{
                background:
                  i <= currentEraIndex ? "var(--color-lifecycle)" : "var(--color-bg-primary)",
                opacity: getEraOpacity(i, currentEraIndex),
              }}
              // eslint-disable-next-line security/detect-object-injection -- era is from the static ERA_ORDER array of Era literals, not user input
              title={`${era}: ${ERA_TRIGGERS[era]}`}
            />
          ))}
        </div>
        <div className="flex justify-between text-xs font-mono text-text-muted mb-md">
          <span>Primitive</span>
          <span>Modern</span>
        </div>

        {/* Tech Tree */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Tech Tree
        </div>
        <div className="flex flex-wrap gap-xs mb-md">
          {techTreeStatus.map((node) => (
            <div
              key={node.name}
              className="px-2 py-px rounded-sm font-mono text-xs"
              style={{
                border: `1px solid ${getTechNodeBorderColor(node.discovered, node.adjacent)}`,
                background: getTechNodeBackground(node.discovered, node.adjacent),
                color: getTechNodeColor(node.discovered, node.adjacent),
              }}
              title={getTechNodeTitle(node)}
            >
              {getTechNodeLabel(node)}
            </div>
          ))}
        </div>

        {/* Global Knowledge */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Global Discoveries ({discoveries.length})
        </div>
        <div className="flex flex-wrap gap-xs mb-md">
          {discoveries.map((d) => (
            <span
              key={d}
              className="px-1.5 py-px bg-success/10 border border-success/30 rounded-sm font-mono text-2xs text-success"
            >
              {d}
            </span>
          ))}
        </div>

        {/* Discovery Events Log */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Discovery History
        </div>
        {discoveryEvents.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
            No discoveries recorded yet
          </div>
        ) : (
          discoveryEvents.map((d, i) => (
            <div
              key={i}
              className="flex gap-sm px-md py-sm border-b border-border-secondary text-xs"
            >
              <span className="font-mono text-text-muted min-w-[60px]">{formatTick(d.tick)}</span>
              <div>
                <span className="text-success font-semibold">{d.knowledge}</span>
                <span className="text-text-secondary mx-sm">by</span>
                <span className="text-text-primary">{d.agentName}</span>
                <span className="text-text-secondary italic ml-sm">({d.method})</span>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

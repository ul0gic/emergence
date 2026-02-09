/**
 * Agent Inspector Panel (Task 4.5.2)
 *
 * List of all agents with search/filter. Click to see deep dive:
 * vitals, personality radar chart, inventory, knowledge, skills,
 * goals, relationships, and memory.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import * as d3 from "d3";

import { useDebounce } from "../hooks/useApi.ts";
import { cn } from "../lib/utils.ts";
import type {
  AgentDetailResponse,
  AgentListItem,
  Personality,
  Resource,
} from "../types/generated/index.ts";
import { formatDecimal, formatNumber, formatResourceName } from "../utils/format.ts";
import { MOCK_AGENTS, MOCK_AGENT_DETAIL } from "../utils/mockData.ts";

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
  return "bg-hunger";
}

interface AgentInspectorProps {
  agents: AgentListItem[];
  onSelectAgent: (id: string) => void;
  selectedAgentId: string | null;
  agentDetail: AgentDetailResponse | null;
  useMock?: boolean;
}

type StatusFilter = "all" | "alive" | "dead";

// ---------------------------------------------------------------------------
// Personality Radar Chart
// ---------------------------------------------------------------------------

function PersonalityRadar({ personality }: { personality: Personality }) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    const size = 160;
    const center = size / 2;
    const radius = 55;
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
  }, [personality]);

  return <svg ref={svgRef} className="radar-chart w-full max-w-[180px]" />;
}

// ---------------------------------------------------------------------------
// Agent Inspector Component
// ---------------------------------------------------------------------------

export default function AgentInspector({
  agents: propAgents,
  onSelectAgent,
  selectedAgentId,
  agentDetail: propDetail,
  useMock = false,
}: AgentInspectorProps) {
  const [searchText, setSearchText] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const debouncedSearch = useDebounce(searchText, 200);

  const agents = useMock ? MOCK_AGENTS : propAgents;
  const agentDetail = useMock ? MOCK_AGENT_DETAIL : propDetail;

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
            <AgentDetail detail={agentDetail} agents={agents} />
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
// Agent Detail View
// ---------------------------------------------------------------------------

function AgentDetail({ detail, agents }: { detail: AgentDetailResponse; agents: AgentListItem[] }) {
  const { agent, state } = detail;

  const agentNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agents) {
      map.set(a.id, a.name);
    }
    return map;
  }, [agents]);

  return (
    <div>
      {/* Header */}
      <div className="flex items-center gap-md mb-md">
        <div>
          <h2 className="text-lg text-text-accent font-mono m-0">{agent.name}</h2>
          <div className="text-xs text-text-muted font-mono">
            Gen {agent.generation} | Born T{formatNumber(agent.born_at_tick)}
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

      {state && (
        <>
          {/* Vitals */}
          <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
            Vitals
          </div>
          <div className="mb-md">
            <VitalBar label="Energy" value={state.energy} max={100} type="energy" />
            <VitalBar label="Health" value={state.health} max={100} type="health" />
            <VitalBar label="Hunger" value={state.hunger} max={100} type="hunger" />
            <div className="flex gap-lg mt-sm text-xs font-mono text-text-secondary">
              <span>Age: {formatNumber(state.age)} ticks</span>
              <span>
                Carry: {totalInventory(state.inventory)}/{state.carry_capacity}
              </span>
            </div>
          </div>

          {/* Personality + Inventory side by side */}
          <div className="flex gap-lg">
            <div>
              <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
                Personality
              </div>
              <PersonalityRadar personality={agent.personality} />
            </div>
            <div className="flex-1">
              <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
                Inventory
              </div>
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
          <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
            Goals
          </div>
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
          <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
            Skills
          </div>
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
          <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
            Knowledge ({state.knowledge.length})
          </div>
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
          <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
            Relationships
          </div>
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
          <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
            Memory ({state.memory.length})
          </div>
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
// Helpers
// ---------------------------------------------------------------------------

function totalInventory(inv: Partial<Record<Resource, number>>): number {
  let total = 0;
  for (const val of Object.values(inv)) {
    total += val ?? 0;
  }
  return total;
}

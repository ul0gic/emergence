/**
 * Social Constructs Panel (Task 6.4.7, Phase 9.7.2-9.7.3)
 *
 * Tabbed panel with 6 sub-views: Religion, Governance, Family,
 * Economy (extended), Crime & Justice, and Civilization Timeline.
 * Fetches data from 5 social construct API endpoints on tick update.
 * The Civilization Timeline aggregates milestones from all constructs
 * to show the arc of emergent civilization.
 */
import { useEffect, useMemo, useRef, useState } from "react";

import * as d3 from "d3";

import { useSocialConstructs } from "../hooks/useApi.ts";
import { cn } from "../lib/utils.ts";
import type {
  BeliefEvent,
  BeliefSystem,
  CivilizationMilestone,
  CivilizationMilestoneCategory,
  CrimeStats,
  EconomicClassification,
  FamilyStats,
  GovernanceInfo,
  LineageNode,
} from "../types/generated/index.ts";
import { formatNumber, formatResourceName, formatTick, getResourceColor } from "../utils/format.ts";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type SubTab = "religion" | "governance" | "family" | "economy" | "crime" | "timeline";

interface SocialConstructsProps {
  /** Current simulation tick -- triggers data refetch when it changes. */
  currentTick?: number;
}

const SUB_TABS: { id: SubTab; label: string }[] = [
  { id: "religion", label: "Religion" },
  { id: "governance", label: "Governance" },
  { id: "family", label: "Family" },
  { id: "economy", label: "Economy" },
  { id: "crime", label: "Crime & Justice" },
  { id: "timeline", label: "Civ Timeline" },
];

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/** Empty state placeholder shown when no data is available for a sub-panel. */
function EmptyState({ label }: { label: string }) {
  return (
    <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
      No {label} data available yet. Waiting for simulation data...
    </div>
  );
}

export default function SocialConstructs({
  currentTick = 0,
}: SocialConstructsProps) {
  const [activeSubTab, setActiveSubTab] = useState<SubTab>("religion");

  // Fetch data from all 5 social construct API endpoints.
  const {
    beliefSystems,
    beliefEvents,
    governance,
    familyStats,
    economicClassification,
    crimeStats,
    loading,
    error,
    refetch,
  } = useSocialConstructs();

  // Refetch when tick changes.
  useEffect(() => {
    if (currentTick > 0) {
      refetch();
    }
    // Only re-run when tick number changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentTick]);

  // Build civilization milestones from all social construct data.
  const milestones = useMemo(
    () => buildCivilizationMilestones(beliefSystems, beliefEvents, governance, familyStats, economicClassification, crimeStats),
    [beliefSystems, beliefEvents, governance, familyStats, economicClassification, crimeStats],
  );

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      {/* Panel header */}
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>Social Constructs</span>
        <span className="text-xs font-normal">
          {loading ? "Loading..." : error ? "Partial data" : "Emergent Patterns"}
        </span>
      </div>

      {/* Sub-tab navigation */}
      <nav className="flex bg-bg-tertiary border-b border-border-primary px-md shrink-0">
        {SUB_TABS.map((tab) => (
          <button
            key={tab.id}
            className={cn(
              "px-md py-sm text-xs font-mono bg-transparent border-0 border-b-2 border-b-transparent cursor-pointer whitespace-nowrap transition-colors duration-150",
              activeSubTab === tab.id
                ? "text-text-accent border-b-text-accent"
                : "text-text-secondary hover:text-text-primary",
            )}
            onClick={() => setActiveSubTab(tab.id)}
          >
            {tab.label}
            {tab.id === "timeline" && milestones.length > 0 && (
              <span className="ml-1 text-2xs text-text-muted">({milestones.length})</span>
            )}
          </button>
        ))}
      </nav>

      {/* Sub-tab content */}
      <div className="flex-1 min-h-0 overflow-y-auto p-md">
        {activeSubTab === "religion" && (
          beliefSystems.length === 0 && beliefEvents.length === 0
            ? <EmptyState label="religion" />
            : <ReligionPanel beliefSystems={beliefSystems} beliefEvents={beliefEvents} />
        )}
        {activeSubTab === "governance" && (
          governance
            ? <GovernancePanel governance={governance} />
            : <EmptyState label="governance" />
        )}
        {activeSubTab === "family" && (
          familyStats
            ? <FamilyPanel familyStats={familyStats} />
            : <EmptyState label="family" />
        )}
        {activeSubTab === "economy" && (
          economicClassification
            ? <EconomyExtendedPanel classification={economicClassification} />
            : <EmptyState label="economic classification" />
        )}
        {activeSubTab === "crime" && (
          crimeStats
            ? <CrimePanel crimeStats={crimeStats} />
            : <EmptyState label="crime & justice" />
        )}
        {activeSubTab === "timeline" && (
          milestones.length === 0
            ? <EmptyState label="civilization timeline" />
            : <CivilizationTimeline milestones={milestones} />
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// 1. Religion Panel
// ---------------------------------------------------------------------------

function ReligionPanel({
  beliefSystems,
  beliefEvents,
}: {
  beliefSystems: BeliefSystem[];
  beliefEvents: BeliefEvent[];
}) {
  const totalAdherents = useMemo(
    () => beliefSystems.reduce((acc, bs) => acc + bs.adherent_count, 0),
    [beliefSystems],
  );

  const sortedEvents = useMemo(
    () => [...beliefEvents].sort((a, b) => b.tick - a.tick),
    [beliefEvents],
  );

  return (
    <>
      {/* Stats */}
      <div className="flex gap-sm mb-md">
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Belief Systems
          </div>
          <div className="text-lg font-bold text-text-accent font-mono">{beliefSystems.length}</div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Total Adherents
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">
            {formatNumber(totalAdherents)}
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Events
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">{beliefEvents.length}</div>
        </div>
      </div>

      {/* Belief system list */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Active Belief Systems
      </div>
      {beliefSystems.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No belief systems have emerged yet
        </div>
      ) : (
        <div className="mb-md">
          {beliefSystems.map((bs) => (
            <div
              key={bs.id}
              className="flex items-center gap-sm px-md py-sm border-b border-border-secondary text-xs"
            >
              <div className="flex-1">
                <div className="text-text-primary font-semibold font-mono">{bs.name}</div>
                <div className="flex gap-xs mt-xs flex-wrap">
                  {bs.themes.map((theme) => (
                    <span
                      key={theme}
                      className="px-1.5 py-px bg-lifecycle/10 border border-lifecycle/30 rounded-sm font-mono text-2xs text-lifecycle"
                    >
                      {theme}
                    </span>
                  ))}
                </div>
              </div>
              <div className="text-right">
                <div className="text-text-primary font-mono font-semibold">{bs.adherent_count}</div>
                <div className="text-2xs text-text-muted">adherents</div>
              </div>
              <div className="text-right min-w-[70px]">
                <div className="text-text-secondary font-mono text-2xs">
                  {formatTick(bs.founded_at_tick)}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Adherent count bar chart */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Adherent Distribution
      </div>
      <AdherentBarChart beliefSystems={beliefSystems} />

      {/* Belief event timeline */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Belief Event History
      </div>
      {sortedEvents.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No belief events recorded yet
        </div>
      ) : (
        sortedEvents.map((ev, i) => (
          <div
            key={`${ev.tick}-${i}`}
            className="flex gap-sm px-md py-sm border-b border-border-secondary text-xs"
          >
            <span className="font-mono text-text-muted min-w-[60px]">{formatTick(ev.tick)}</span>
            <span
              className={cn(
                "inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold min-w-[70px] justify-center",
                beliefEventBadgeClasses(ev.event_type),
              )}
            >
              {ev.event_type}
            </span>
            <span className="text-text-secondary flex-1">{ev.description}</span>
          </div>
        ))
      )}
    </>
  );
}

function beliefEventBadgeClasses(eventType: BeliefEvent["event_type"]): string {
  switch (eventType) {
    case "founded":
      return "bg-success/15 text-success";
    case "schism":
      return "bg-danger/15 text-danger";
    case "merged":
      return "bg-info/15 text-info";
    case "converted":
      return "bg-lifecycle/15 text-lifecycle";
  }
}

// ---------------------------------------------------------------------------
// Adherent Horizontal Bar Chart (D3)
// ---------------------------------------------------------------------------

function AdherentBarChart({ beliefSystems }: { beliefSystems: BeliefSystem[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (beliefSystems.length === 0) {
      svg
        .attr("viewBox", "0 0 500 80")
        .append("text")
        .attr("x", 250)
        .attr("y", 40)
        .attr("text-anchor", "middle")
        .attr("fill", "#484f58")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("No belief systems");
      return;
    }

    const width = 500;
    const barHeight = 24;
    const gap = 4;
    const height = beliefSystems.length * (barHeight + gap) + 16;
    const margin = { top: 8, right: 50, bottom: 8, left: 110 };
    const innerW = width - margin.left - margin.right;

    svg.attr("viewBox", `0 0 ${width} ${height}`);

    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const maxVal = d3.max(beliefSystems, (d) => d.adherent_count) ?? 1;
    const x = d3.scaleLinear().domain([0, maxVal]).range([0, innerW]);

    const chartColors = [
      "var(--color-chart-4)",
      "var(--color-chart-1)",
      "var(--color-chart-5)",
      "var(--color-chart-3)",
      "var(--color-chart-2)",
    ];

    beliefSystems.forEach((bs, i) => {
      const y = i * (barHeight + gap);
      const color = chartColors[i % chartColors.length] ?? "var(--color-chart-1)";

      // Bar
      g.append("rect")
        .attr("x", 0)
        .attr("y", y)
        .attr("width", x(bs.adherent_count))
        .attr("height", barHeight)
        .attr("fill", color)
        .attr("opacity", 0.7)
        .attr("rx", 2);

      // Label (left)
      g.append("text")
        .attr("x", -8)
        .attr("y", y + barHeight / 2)
        .attr("dy", "0.35em")
        .attr("text-anchor", "end")
        .attr("fill", "var(--color-text-secondary)")
        .attr("font-size", "10px")
        .attr("font-family", "var(--font-mono)")
        .text(bs.name);

      // Count (right of bar)
      g.append("text")
        .attr("x", x(bs.adherent_count) + 6)
        .attr("y", y + barHeight / 2)
        .attr("dy", "0.35em")
        .attr("fill", "var(--color-text-primary)")
        .attr("font-size", "11px")
        .attr("font-family", "var(--font-mono)")
        .attr("font-weight", "600")
        .text(String(bs.adherent_count));
    });
  }, [beliefSystems]);

  return (
    <div className="chart-container mb-sm">
      <svg ref={svgRef} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// 2. Governance Panel
// ---------------------------------------------------------------------------

function GovernancePanel({ governance }: { governance: GovernanceInfo }) {
  const stabilityPct = Math.round(governance.stability_score * 100);
  const challengedPct = 100 - stabilityPct;

  const sortedEvents = useMemo(
    () => [...governance.recent_events].sort((a, b) => b.tick - a.tick),
    [governance.recent_events],
  );

  return (
    <>
      {/* Type and stability */}
      <div className="flex gap-sm mb-md">
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Government Type
          </div>
          <div className="mt-xs">
            <span
              className={cn(
                "inline-flex items-center px-2 py-px rounded-[10px] text-xs font-mono font-semibold",
                governanceTypeBadgeClasses(governance.governance_type),
              )}
            >
              {governance.governance_type}
            </span>
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Stability
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">{stabilityPct}%</div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Leaders
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">
            {governance.leaders.length}
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Rules
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">
            {governance.rules.length}
          </div>
        </div>
      </div>

      {/* Stability bar */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Stability Indicator
      </div>
      <div className="mb-md">
        <div className="flex justify-between text-2xs font-mono text-text-secondary mb-xs">
          <span>Stable ({stabilityPct}%)</span>
          <span>Challenged ({challengedPct}%)</span>
        </div>
        <div className="h-3 bg-bg-primary rounded-sm overflow-hidden flex">
          <div
            className="h-full bg-success rounded-l-sm transition-[width] duration-300 ease-in-out"
            style={{ width: `${stabilityPct}%` }}
          />
          <div
            className="h-full bg-danger rounded-r-sm transition-[width] duration-300 ease-in-out"
            style={{ width: `${challengedPct}%` }}
          />
        </div>
      </div>

      {/* Leadership */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Leadership
      </div>
      {governance.leaders.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No leaders established
        </div>
      ) : (
        <div className="mb-md">
          {governance.leaders.map((leader) => (
            <div
              key={leader.agent_id}
              className="flex items-center gap-sm px-md py-sm border-b border-border-secondary text-xs"
            >
              <span className="text-text-accent font-semibold font-mono">{leader.agent_name}</span>
              <span className="inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold bg-info/15 text-info">
                {leader.role}
              </span>
              <span className="text-text-muted ml-auto font-mono text-2xs">
                since {formatTick(leader.since_tick)}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Rules */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Declared Rules
      </div>
      {governance.rules.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No rules declared yet
        </div>
      ) : (
        <div className="mb-md">
          {governance.rules.map((rule, i) => (
            <div
              key={i}
              className="flex items-start gap-sm px-md py-sm border-b border-border-secondary text-xs"
            >
              <span className="text-text-muted font-mono">{i + 1}.</span>
              <span className="text-text-secondary">{rule}</span>
            </div>
          ))}
        </div>
      )}

      {/* Recent governance events */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Recent Events
      </div>
      {sortedEvents.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No governance events recorded
        </div>
      ) : (
        sortedEvents.map((ev, i) => (
          <div
            key={`${ev.tick}-${i}`}
            className="flex gap-sm px-md py-sm border-b border-border-secondary text-xs"
          >
            <span className="font-mono text-text-muted min-w-[60px]">{formatTick(ev.tick)}</span>
            <span
              className={cn(
                "inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold min-w-[80px] justify-center",
                governanceEventBadgeClasses(ev.event_type),
              )}
            >
              {ev.event_type}
            </span>
            <span className="text-text-secondary flex-1">{ev.description}</span>
          </div>
        ))
      )}
    </>
  );
}

function governanceTypeBadgeClasses(govType: GovernanceInfo["governance_type"]): string {
  switch (govType) {
    case "Anarchy":
      return "bg-danger/15 text-danger";
    case "Chieftainship":
      return "bg-warning/15 text-warning";
    case "Council":
      return "bg-info/15 text-info";
    case "Monarchy":
      return "bg-lifecycle/15 text-lifecycle";
    case "Democracy":
      return "bg-success/15 text-success";
    case "Oligarchy":
      return "bg-chart-3/15 text-chart-3";
    case "Theocracy":
      return "bg-chart-4/15 text-chart-4";
  }
}

function governanceEventBadgeClasses(
  eventType: GovernanceInfo["recent_events"][number]["event_type"],
): string {
  switch (eventType) {
    case "election":
      return "bg-success/15 text-success";
    case "coup":
      return "bg-danger/15 text-danger";
    case "declaration":
      return "bg-info/15 text-info";
    case "succession":
      return "bg-warning/15 text-warning";
    case "reform":
      return "bg-lifecycle/15 text-lifecycle";
  }
}

// ---------------------------------------------------------------------------
// 3. Family Panel
// ---------------------------------------------------------------------------

function FamilyPanel({ familyStats }: { familyStats: FamilyStats }) {
  const [selectedFamilyId, setSelectedFamilyId] = useState<string | null>(
    familyStats.families[0]?.id ?? null,
  );

  // Build lineage subset for the selected family.
  const selectedFamilyLineage = useMemo(() => {
    const family = familyStats.families.find((f) => f.id === selectedFamilyId);
    if (!family) return [];
    const memberSet = new Set(family.members);
    return familyStats.lineage.filter((node) => memberSet.has(node.agent_id));
  }, [familyStats, selectedFamilyId]);

  return (
    <>
      {/* Stats */}
      <div className="flex gap-sm mb-md">
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Families
          </div>
          <div className="text-lg font-bold text-text-accent font-mono">
            {familyStats.unit_count}
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Avg Size
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">
            {familyStats.avg_size.toFixed(1)}
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Marriages
          </div>
          <div className="text-lg font-bold text-success font-mono">
            {familyStats.marriage_count}
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Divorces
          </div>
          <div className="text-lg font-bold text-danger font-mono">{familyStats.divorce_count}</div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Orphans
          </div>
          <div className="text-lg font-bold text-warning font-mono">{familyStats.orphan_count}</div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Max Depth
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">
            {familyStats.longest_lineage}
          </div>
        </div>
      </div>

      {/* Family unit list */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Family Units
      </div>
      {familyStats.families.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No family units formed yet
        </div>
      ) : (
        <div className="mb-md">
          {familyStats.families.map((family) => (
            <button
              key={family.id}
              className={cn(
                "flex items-center gap-sm px-md py-sm border-b border-border-secondary text-xs w-full text-left bg-transparent cursor-pointer transition-colors duration-150",
                selectedFamilyId === family.id
                  ? "bg-info/10 border-l-2 border-l-text-accent"
                  : "hover:bg-bg-tertiary",
              )}
              onClick={() => setSelectedFamilyId(family.id)}
            >
              <span className="text-text-primary font-semibold font-mono">{family.name}</span>
              <span className="text-text-muted font-mono text-2xs">
                {family.members.length} members
              </span>
              <span className="text-text-muted font-mono text-2xs ml-auto">
                {formatTick(family.formed_at_tick)}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Lineage tree */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Lineage Tree
        {selectedFamilyId && (
          <span className="text-text-secondary ml-sm normal-case">
            ({familyStats.families.find((f) => f.id === selectedFamilyId)?.name ?? "Unknown"})
          </span>
        )}
      </div>
      <LineageTree lineage={selectedFamilyLineage} />
    </>
  );
}

// ---------------------------------------------------------------------------
// Lineage Tree (D3 Tree Layout)
// ---------------------------------------------------------------------------

interface TreeDatum {
  name: string;
  agentId: string;
  alive: boolean;
  children?: TreeDatum[];
}

function LineageTree({ lineage }: { lineage: LineageNode[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  const treeData = useMemo((): TreeDatum | null => {
    if (lineage.length === 0) return null;

    // Build a lookup map.
    const nodeMap = new Map<string, LineageNode>();
    for (const node of lineage) {
      nodeMap.set(node.agent_id, node);
    }

    // Find root nodes (nodes with no parents in the lineage set).
    const roots = lineage.filter(
      (n) => (!n.parent_a || !nodeMap.has(n.parent_a)) && (!n.parent_b || !nodeMap.has(n.parent_b)),
    );

    // Build a hierarchical structure. Use first root as primary.
    function buildNode(node: LineageNode): TreeDatum {
      const childNodes = lineage.filter(
        (n) => n.parent_a === node.agent_id || n.parent_b === node.agent_id,
      );

      // Deduplicate children that share both parents.
      const seen = new Set<string>();
      const uniqueChildren: LineageNode[] = [];
      for (const child of childNodes) {
        if (!seen.has(child.agent_id)) {
          seen.add(child.agent_id);
          uniqueChildren.push(child);
        }
      }

      return {
        name: node.agent_name,
        agentId: node.agent_id,
        alive: node.alive,
        children: uniqueChildren.length > 0 ? uniqueChildren.map(buildNode) : undefined,
      };
    }

    // If multiple roots, create a synthetic root.
    if (roots.length === 1 && roots[0]) {
      return buildNode(roots[0]);
    }

    return {
      name: "Family",
      agentId: "",
      alive: true,
      children: roots.map(buildNode),
    };
  }, [lineage]);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (!treeData) {
      svg
        .attr("viewBox", "0 0 500 80")
        .append("text")
        .attr("x", 250)
        .attr("y", 40)
        .attr("text-anchor", "middle")
        .attr("fill", "#484f58")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("Select a family to view lineage");
      return;
    }

    const width = 500;
    const height = 160;
    const margin = { top: 20, right: 40, bottom: 20, left: 40 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const root = d3.hierarchy<TreeDatum>(treeData);
    const treeLayout = d3.tree<TreeDatum>().size([innerW, innerH]);
    treeLayout(root);

    // Links.
    g.selectAll(".tree-link")
      .data(root.links())
      .join("path")
      .attr("fill", "none")
      .attr("stroke", "var(--color-border-primary)")
      .attr("stroke-width", 1.5)
      .attr(
        "d",
        d3
          .linkVertical<d3.HierarchyLink<TreeDatum>, d3.HierarchyPointNode<TreeDatum>>()
          .x((d) => d.x ?? 0)
          .y((d) => d.y ?? 0) as unknown as (d: d3.HierarchyLink<TreeDatum>) => string | null,
      );

    // Nodes.
    const nodeGroups = g
      .selectAll(".tree-node")
      .data(root.descendants())
      .join("g")
      .attr("transform", (d) => `translate(${d.x},${d.y})`);

    nodeGroups
      .append("circle")
      .attr("r", 6)
      .attr("fill", (d) => (d.data.alive ? "var(--color-text-accent)" : "var(--color-text-muted)"))
      .attr("fill-opacity", (d) => (d.data.alive ? 0.3 : 0.15))
      .attr("stroke", (d) =>
        d.data.alive ? "var(--color-text-accent)" : "var(--color-text-muted)",
      )
      .attr("stroke-width", 1.5);

    nodeGroups
      .append("text")
      .attr("dy", -12)
      .attr("text-anchor", "middle")
      .attr("fill", (d) => (d.data.alive ? "var(--color-text-primary)" : "var(--color-text-muted)"))
      .attr("font-size", "10px")
      .attr("font-family", "var(--font-mono)")
      .text((d) => d.data.name);
  }, [treeData]);

  return (
    <div className="chart-container mb-md">
      <svg ref={svgRef} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// 4. Economy Extended Panel
// ---------------------------------------------------------------------------

function EconomyExtendedPanel({ classification }: { classification: EconomicClassification }) {
  return (
    <>
      {/* Stats */}
      <div className="flex gap-sm mb-md">
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Economic Model
          </div>
          <div className="mt-xs">
            <span
              className={cn(
                "inline-flex items-center px-2 py-px rounded-[10px] text-xs font-mono font-semibold",
                economicModelBadgeClasses(classification.model_type),
              )}
            >
              {classification.model_type}
            </span>
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Trade Volume
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">
            {formatNumber(classification.trade_volume)}
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Market Locations
          </div>
          <div className="text-lg font-bold text-text-primary font-mono">
            {classification.market_locations.length}
          </div>
        </div>
      </div>

      {/* Currency detection */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Currency Detection
      </div>
      <div className="flex items-center gap-sm px-md py-sm bg-bg-tertiary rounded-sm mb-md text-sm font-mono">
        {classification.currency_resource ? (
          <>
            <span className="text-text-secondary">Currency:</span>
            <span className="text-text-accent font-semibold">
              {formatResourceName(classification.currency_resource)}
            </span>
            <span className="text-text-muted">|</span>
            <span className="text-text-secondary">Adoption:</span>
            <span className="text-text-primary font-semibold">
              {classification.currency_adoption_pct.toFixed(1)}%
            </span>
          </>
        ) : (
          <span className="text-text-muted">No currency detected -- pure barter economy</span>
        )}
      </div>

      {/* Trade volume sparkline */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Trade Volume Trend
      </div>
      <TradeVolumeChart history={classification.trade_volume_history} />

      {/* Market hotspots */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Market Hotspots
      </div>
      {classification.market_locations.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No market activity detected
        </div>
      ) : (
        <div className="text-xs font-mono">
          <div className="grid grid-cols-[1fr_80px_100px] px-sm py-xs text-text-muted border-b border-border-primary">
            <span>Location</span>
            <span className="text-right">Volume</span>
            <span className="text-right">Primary</span>
          </div>
          {classification.market_locations.map((loc) => (
            <div
              key={loc.location_id}
              className="grid grid-cols-[1fr_80px_100px] px-sm py-xs border-b border-border-secondary"
            >
              <span className="text-text-primary">{loc.location_name}</span>
              <span className="text-right text-text-primary font-semibold">
                {formatNumber(loc.trade_volume)}
              </span>
              <span className="text-right flex items-center justify-end gap-1">
                <span
                  className="w-2 h-2 rounded-sm inline-block"
                  style={{ background: getResourceColor(loc.primary_resource) }}
                />
                {formatResourceName(loc.primary_resource)}
              </span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

function economicModelBadgeClasses(modelType: EconomicClassification["model_type"]): string {
  switch (modelType) {
    case "Subsistence":
      return "bg-danger/15 text-danger";
    case "Gift":
      return "bg-lifecycle/15 text-lifecycle";
    case "Barter":
      return "bg-warning/15 text-warning";
    case "Currency":
      return "bg-success/15 text-success";
    case "Market":
      return "bg-info/15 text-info";
    case "Mixed":
      return "bg-chart-4/15 text-chart-4";
  }
}

// ---------------------------------------------------------------------------
// Trade Volume Chart (D3)
// ---------------------------------------------------------------------------

function TradeVolumeChart({ history }: { history: { tick: number; volume: number }[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  const sorted = useMemo(() => [...history].sort((a, b) => a.tick - b.tick), [history]);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (sorted.length < 2) {
      svg
        .attr("viewBox", "0 0 500 100")
        .append("text")
        .attr("x", 250)
        .attr("y", 50)
        .attr("text-anchor", "middle")
        .attr("fill", "#484f58")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("Waiting for trade data...");
      return;
    }

    const width = 500;
    const height = 100;
    const margin = { top: 8, right: 10, bottom: 25, left: 36 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const x = d3
      .scaleBand()
      .domain(sorted.map((d) => String(d.tick)))
      .range([0, innerW])
      .padding(0.25);

    const y = d3
      .scaleLinear()
      .domain([0, d3.max(sorted, (d) => d.volume) ?? 1])
      .range([innerH, 0]);

    // Axes
    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(d3.axisBottom(x).tickFormat((d) => `T${d}`))
      .selectAll("text")
      .attr("font-size", "8px")
      .attr("transform", "rotate(-35)")
      .attr("text-anchor", "end");

    g.append("g").attr("class", "axis").call(d3.axisLeft(y).ticks(4));

    // Bars
    g.selectAll("rect")
      .data(sorted)
      .join("rect")
      .attr("x", (d) => x(String(d.tick)) ?? 0)
      .attr("y", (d) => y(d.volume))
      .attr("width", x.bandwidth())
      .attr("height", (d) => innerH - y(d.volume))
      .attr("fill", "var(--color-chart-3)")
      .attr("opacity", 0.7)
      .attr("rx", 1);
  }, [sorted]);

  return (
    <div className="chart-container mb-sm">
      <svg ref={svgRef} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// 5. Crime & Justice Panel
// ---------------------------------------------------------------------------

function CrimePanel({ crimeStats }: { crimeStats: CrimeStats }) {
  const crimeRatePct = (crimeStats.crime_rate * 100).toFixed(1);
  const detectionPct = Math.round(crimeStats.detection_rate * 100);
  const punishmentPct = Math.round(crimeStats.punishment_rate * 100);

  return (
    <>
      {/* Stats */}
      <div className="flex gap-sm mb-md">
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Justice System
          </div>
          <div className="mt-xs">
            <span
              className={cn(
                "inline-flex items-center px-2 py-px rounded-[10px] text-xs font-mono font-semibold",
                justiceTypeBadgeClasses(crimeStats.justice_type),
              )}
            >
              {crimeStats.justice_type}
            </span>
          </div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Crime Rate
          </div>
          <div className="text-lg font-bold text-danger font-mono">{crimeRatePct}%</div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Detection
          </div>
          <div className="text-lg font-bold text-warning font-mono">{detectionPct}%</div>
        </div>
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Punishment
          </div>
          <div className="text-lg font-bold text-info font-mono">{punishmentPct}%</div>
        </div>
      </div>

      {/* Detection/Punishment rate bars */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Rate Indicators
      </div>
      <div className="mb-md px-md">
        <div className="mb-sm">
          <div className="flex justify-between text-2xs font-mono text-text-secondary mb-xs">
            <span>Detection Rate</span>
            <span>{detectionPct}%</span>
          </div>
          <div className="h-2 bg-bg-primary rounded-sm overflow-hidden">
            <div
              className="h-full bg-warning rounded-sm transition-[width] duration-300 ease-in-out"
              style={{ width: `${detectionPct}%` }}
            />
          </div>
        </div>
        <div>
          <div className="flex justify-between text-2xs font-mono text-text-secondary mb-xs">
            <span>Punishment Rate</span>
            <span>{punishmentPct}%</span>
          </div>
          <div className="h-2 bg-bg-primary rounded-sm overflow-hidden">
            <div
              className="h-full bg-info rounded-sm transition-[width] duration-300 ease-in-out"
              style={{ width: `${punishmentPct}%` }}
            />
          </div>
        </div>
      </div>

      {/* Crime rate trend */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Crime Rate Trend
      </div>
      <CrimeRateChart history={crimeStats.crime_rate_history} />

      {/* Most common crimes */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Most Common Crimes
      </div>
      {crimeStats.common_crimes.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No crimes recorded
        </div>
      ) : (
        <div className="mb-md">
          {crimeStats.common_crimes.map((crime) => (
            <div
              key={crime.crime_type}
              className="flex items-center gap-sm px-md py-sm border-b border-border-secondary text-xs"
            >
              <span className="text-text-primary font-mono flex-1">{crime.crime_type}</span>
              <span className="text-danger font-mono font-semibold">{crime.count}</span>
            </div>
          ))}
        </div>
      )}

      {/* Serial offenders */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Serial Offenders
      </div>
      {crimeStats.serial_offenders.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No serial offenders
        </div>
      ) : (
        <div className="mb-md">
          {crimeStats.serial_offenders.map((offender) => (
            <div
              key={offender.agent_id}
              className="flex items-center gap-sm px-md py-sm border-b border-border-secondary text-xs"
            >
              <span className="text-text-accent font-semibold font-mono">
                {offender.agent_name}
              </span>
              <span className="text-danger font-mono">{offender.offense_count} offenses</span>
              <span className="text-text-muted font-mono text-2xs ml-auto">
                last: {formatTick(offender.last_offense_tick)}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Crime hotspots */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Crime Hotspots
      </div>
      {crimeStats.hotspots.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[60px]">
          No crime hotspots identified
        </div>
      ) : (
        <div className="mb-md">
          {crimeStats.hotspots.map((hotspot) => (
            <div
              key={hotspot.location_id}
              className="flex items-center gap-sm px-md py-sm border-b border-border-secondary text-xs"
            >
              <span className="text-text-primary font-mono flex-1">{hotspot.location_name}</span>
              <span className="text-danger font-mono font-semibold">
                {hotspot.crime_count} incidents
              </span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

function justiceTypeBadgeClasses(justiceType: CrimeStats["justice_type"]): string {
  switch (justiceType) {
    case "None":
      return "bg-danger/15 text-danger";
    case "Vigilante":
      return "bg-chart-5/15 text-chart-5";
    case "Elder":
      return "bg-warning/15 text-warning";
    case "Council":
      return "bg-info/15 text-info";
    case "Codified":
      return "bg-lifecycle/15 text-lifecycle";
    case "Institutional":
      return "bg-success/15 text-success";
  }
}

// ---------------------------------------------------------------------------
// Crime Rate Bar Chart (D3)
// ---------------------------------------------------------------------------

function CrimeRateChart({ history }: { history: { tick: number; rate: number }[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  const sorted = useMemo(() => [...history].sort((a, b) => a.tick - b.tick), [history]);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (sorted.length < 2) {
      svg
        .attr("viewBox", "0 0 500 100")
        .append("text")
        .attr("x", 250)
        .attr("y", 50)
        .attr("text-anchor", "middle")
        .attr("fill", "#484f58")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("Waiting for crime data...");
      return;
    }

    const width = 500;
    const height = 100;
    const margin = { top: 8, right: 10, bottom: 25, left: 36 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const x = d3
      .scaleBand()
      .domain(sorted.map((d) => String(d.tick)))
      .range([0, innerW])
      .padding(0.25);

    const y = d3
      .scaleLinear()
      .domain([0, d3.max(sorted, (d) => d.rate) ?? 0.1])
      .range([innerH, 0]);

    // Axes
    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(d3.axisBottom(x).tickFormat((d) => `T${d}`))
      .selectAll("text")
      .attr("font-size", "8px")
      .attr("transform", "rotate(-35)")
      .attr("text-anchor", "end");

    g.append("g")
      .attr("class", "axis")
      .call(
        d3
          .axisLeft(y)
          .ticks(4)
          .tickFormat((d) => `${(Number(d) * 100).toFixed(0)}%`),
      );

    // Bars
    g.selectAll("rect")
      .data(sorted)
      .join("rect")
      .attr("x", (d) => x(String(d.tick)) ?? 0)
      .attr("y", (d) => y(d.rate))
      .attr("width", x.bandwidth())
      .attr("height", (d) => innerH - y(d.rate))
      .attr("fill", "var(--color-danger)")
      .attr("opacity", 0.6)
      .attr("rx", 1);
  }, [sorted]);

  return (
    <div className="chart-container mb-sm">
      <svg ref={svgRef} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// 6. Civilization Timeline (Phase 9.7.3)
// ---------------------------------------------------------------------------

/** Category badge styling for milestone entries. */
function milestoneCategoryClasses(category: CivilizationMilestoneCategory): string {
  switch (category) {
    case "belief":
      return "bg-lifecycle/15 text-lifecycle";
    case "governance":
      return "bg-info/15 text-info";
    case "family":
      return "bg-success/15 text-success";
    case "economy":
      return "bg-warning/15 text-warning";
    case "crime":
      return "bg-danger/15 text-danger";
  }
}

/** Category label for display. */
function milestoneCategoryLabel(category: CivilizationMilestoneCategory): string {
  switch (category) {
    case "belief":
      return "BELIEF";
    case "governance":
      return "GOVERNANCE";
    case "family":
      return "FAMILY";
    case "economy":
      return "ECONOMY";
    case "crime":
      return "CRIME";
  }
}

/**
 * Build civilization milestones from all social construct data.
 *
 * Each milestone represents a significant "first" or emergence event:
 * - First belief system founded
 * - Governance type detected
 * - First family formed / first marriage
 * - Economic model classification
 * - First crime detected / justice system emerged
 */
function buildCivilizationMilestones(
  beliefSystems: BeliefSystem[],
  beliefEvents: BeliefEvent[],
  governance: GovernanceInfo | null,
  familyStats: FamilyStats | null,
  economicClassification: EconomicClassification | null,
  crimeStats: CrimeStats | null,
): CivilizationMilestone[] {
  const milestones: CivilizationMilestone[] = [];

  // Belief milestones: each belief system founding is a milestone.
  for (const bs of beliefSystems) {
    milestones.push({
      tick: bs.founded_at_tick,
      category: "belief",
      label: `Belief system founded: ${bs.name}`,
      description: `A belief system around "${bs.themes.join(", ")}" emerged with ${bs.adherent_count} adherents`,
    });
  }

  // Belief events (schisms, mergers, conversions).
  for (const ev of beliefEvents) {
    if (ev.event_type !== "founded") {
      milestones.push({
        tick: ev.tick,
        category: "belief",
        label: `Belief ${ev.event_type}: ${ev.belief_system_name}`,
        description: ev.description,
      });
    }
  }

  // Governance milestones.
  if (governance && governance.governance_type !== "Anarchy") {
    // Use earliest leader's since_tick as founding tick.
    const foundingTick = governance.leaders.length > 0
      ? Math.min(...governance.leaders.map((l) => l.since_tick))
      : 0;

    milestones.push({
      tick: foundingTick,
      category: "governance",
      label: `Governance established: ${governance.governance_type}`,
      description: `${governance.governance_type} governance with ${governance.leaders.length} leader${governance.leaders.length === 1 ? "" : "s"} and ${governance.rules.length} rule${governance.rules.length === 1 ? "" : "s"}`,
    });

    // Each governance event is also a milestone.
    for (const ev of governance.recent_events) {
      milestones.push({
        tick: ev.tick,
        category: "governance",
        label: `Governance ${ev.event_type}`,
        description: ev.description,
      });
    }
  }

  // Family milestones.
  if (familyStats && familyStats.unit_count > 0) {
    // Find earliest family formation tick.
    const earliestFamily = familyStats.families.reduce<{ name: string; tick: number } | null>(
      (earliest, f) =>
        earliest === null || f.formed_at_tick < earliest.tick
          ? { name: f.name, tick: f.formed_at_tick }
          : earliest,
      null,
    );

    if (earliestFamily) {
      milestones.push({
        tick: earliestFamily.tick,
        category: "family",
        label: `First family formed: ${earliestFamily.name}`,
        description: `${familyStats.unit_count} family unit${familyStats.unit_count === 1 ? "" : "s"} now exist`,
      });
    }

    if (familyStats.marriage_count > 0) {
      // Approximate first marriage tick from earliest family.
      const marriageTick = earliestFamily?.tick ?? 0;
      milestones.push({
        tick: marriageTick,
        category: "family",
        label: "First marriage",
        description: `${familyStats.marriage_count} marriage${familyStats.marriage_count === 1 ? "" : "s"} recorded`,
      });
    }

    if (familyStats.longest_lineage > 1) {
      milestones.push({
        tick: 0,
        category: "family",
        label: `Lineage depth: ${familyStats.longest_lineage} generations`,
        description: `Multi-generational families have emerged`,
      });
    }
  }

  // Economy milestones.
  if (economicClassification && economicClassification.model_type !== "Subsistence") {
    milestones.push({
      tick: 0,
      category: "economy",
      label: `Economy classified: ${economicClassification.model_type}`,
      description: `Trade volume: ${formatNumber(economicClassification.trade_volume)} across ${economicClassification.market_locations.length} market${economicClassification.market_locations.length === 1 ? "" : "s"}`,
    });

    if (economicClassification.currency_resource) {
      milestones.push({
        tick: 0,
        category: "economy",
        label: `Currency detected: ${formatResourceName(economicClassification.currency_resource)}`,
        description: `${economicClassification.currency_adoption_pct.toFixed(1)}% adoption rate`,
      });
    }
  }

  // Crime milestones.
  if (crimeStats && crimeStats.crime_rate > 0) {
    milestones.push({
      tick: 0,
      category: "crime",
      label: "Crime detected in society",
      description: `Crime rate: ${(crimeStats.crime_rate * 100).toFixed(1)}% with ${crimeStats.common_crimes.length} crime type${crimeStats.common_crimes.length === 1 ? "" : "s"}`,
    });

    if (crimeStats.justice_type !== "None") {
      milestones.push({
        tick: 0,
        category: "crime",
        label: `Justice system: ${crimeStats.justice_type}`,
        description: `Detection rate: ${Math.round(crimeStats.detection_rate * 100)}%, Punishment rate: ${Math.round(crimeStats.punishment_rate * 100)}%`,
      });
    }
  }

  // Sort by tick ascending, then by category for stable order within same tick.
  milestones.sort((a, b) => {
    if (a.tick !== b.tick) return a.tick - b.tick;
    return a.category.localeCompare(b.category);
  });

  return milestones;
}

/**
 * Civilization Timeline -- cross-tab view showing the arc of emergent civilization.
 *
 * Displays a chronological list of milestone events from all social construct
 * categories: beliefs, governance, family, economy, and crime.
 */
function CivilizationTimeline({ milestones }: { milestones: CivilizationMilestone[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  // Category summary counts.
  const categoryCounts = useMemo(() => {
    const counts: Record<CivilizationMilestoneCategory, number> = {
      belief: 0,
      governance: 0,
      family: 0,
      economy: 0,
      crime: 0,
    };
    for (const m of milestones) {
      counts[m.category] += 1;
    }
    return counts;
  }, [milestones]);

  // D3 timeline visualization.
  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (milestones.length === 0) return;

    const width = 600;
    const rowHeight = 20;
    const height = Math.max(80, milestones.length * rowHeight + 40);
    const margin = { top: 20, right: 20, bottom: 10, left: 70 };

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    // Draw vertical timeline axis line.
    g.append("line")
      .attr("x1", 0)
      .attr("y1", 0)
      .attr("x2", 0)
      .attr("y2", milestones.length * rowHeight)
      .attr("stroke", "var(--color-border-primary)")
      .attr("stroke-width", 2);

    // Category colors for D3 markers.
    const categoryColors: Record<CivilizationMilestoneCategory, string> = {
      belief: "var(--color-lifecycle)",
      governance: "var(--color-info)",
      family: "var(--color-success)",
      economy: "var(--color-warning)",
      crime: "var(--color-danger)",
    };

    // Draw milestone markers.
    milestones.forEach((m, i) => {
      const y = i * rowHeight + rowHeight / 2;

      // Dot on the timeline.
      g.append("circle")
        .attr("cx", 0)
        .attr("cy", y)
        .attr("r", 5)
        .attr("fill", categoryColors[m.category])
        .attr("fill-opacity", 0.3)
        .attr("stroke", categoryColors[m.category])
        .attr("stroke-width", 1.5);

      // Tick label.
      g.append("text")
        .attr("x", -8)
        .attr("y", y)
        .attr("dy", "0.35em")
        .attr("text-anchor", "end")
        .attr("fill", "var(--color-text-muted)")
        .attr("font-size", "9px")
        .attr("font-family", "var(--font-mono)")
        .text(m.tick > 0 ? `T${m.tick}` : "");

      // Connecting line from dot to label.
      g.append("line")
        .attr("x1", 8)
        .attr("y1", y)
        .attr("x2", 16)
        .attr("y2", y)
        .attr("stroke", "var(--color-border-secondary)")
        .attr("stroke-width", 1);

      // Milestone label.
      g.append("text")
        .attr("x", 20)
        .attr("y", y)
        .attr("dy", "0.35em")
        .attr("fill", "var(--color-text-primary)")
        .attr("font-size", "10px")
        .attr("font-family", "var(--font-mono)")
        .text(m.label);
    });
  }, [milestones]);

  return (
    <>
      {/* Category summary */}
      <div className="flex gap-sm mb-md">
        <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Milestones
          </div>
          <div className="text-lg font-bold text-text-accent font-mono">{milestones.length}</div>
        </div>
        {(["belief", "governance", "family", "economy", "crime"] as const).map((cat) => (
          <div key={cat} className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              {milestoneCategoryLabel(cat)}
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">{categoryCounts[cat]}</div>
          </div>
        ))}
      </div>

      {/* D3 timeline visualization */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Civilization Arc
      </div>
      <div className="chart-container mb-md">
        <svg ref={svgRef} />
      </div>

      {/* Detailed milestone list */}
      <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
        Milestone Log
      </div>
      {milestones.map((m, i) => (
        <div
          key={`${m.tick}-${m.category}-${i}`}
          className="flex gap-sm px-md py-sm border-b border-border-secondary text-xs"
        >
          <span className="font-mono text-text-muted min-w-[60px]">
            {m.tick > 0 ? formatTick(m.tick) : "--"}
          </span>
          <span
            className={cn(
              "inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold min-w-[80px] justify-center",
              milestoneCategoryClasses(m.category),
            )}
          >
            {milestoneCategoryLabel(m.category)}
          </span>
          <div className="flex-1">
            <div className="text-text-primary">{m.label}</div>
            <div className="text-text-muted text-2xs mt-0.5">{m.description}</div>
          </div>
        </div>
      ))}
    </>
  );
}

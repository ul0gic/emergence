/**
 * Population Tracker Panel (Task 4.5.6, Phase 9.6, Phase 10.1.6)
 *
 * Line chart of population over time. Average age display.
 * Generation distribution. Death causes breakdown (pie chart + table).
 * Lifespan distribution histogram by generation.
 * Sex ratio display. Oldest living agent highlight.
 */
import { useEffect, useMemo, useRef, useState } from "react";

import * as d3 from "d3";

import type { AgentListItem, PopulationStats, TickBroadcast } from "../types/generated/index.ts";
import { formatDecimal, formatNumber, formatTick } from "../utils/format.ts";

interface PopulationTrackerProps {
  populationStats: PopulationStats | null;
  agents: AgentListItem[];
  tickHistory: TickBroadcast[];
}

// ---------------------------------------------------------------------------
// Death cause colors (colorblind-safe palette)
// ---------------------------------------------------------------------------

const DEATH_CAUSE_COLORS: Record<string, string> = {
  starvation: "var(--color-chart-5)",
  dehydration: "var(--color-chart-1)",
  old_age: "var(--color-chart-4)",
  combat: "var(--color-danger)",
  disease: "var(--color-chart-3)",
  exposure: "var(--color-chart-6)",
  unknown: "var(--color-text-muted)",
};

function getDeathCauseColor(cause: string): string {
  const normalized = cause.toLowerCase().replace(/\s+/g, "_");
  return DEATH_CAUSE_COLORS[normalized] ?? "var(--color-chart-7)";
}

function formatCauseName(cause: string): string {
  return cause
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

// ---------------------------------------------------------------------------
// Generation colors for histogram
// ---------------------------------------------------------------------------

const GEN_COLORS = [
  "var(--color-chart-1)",
  "var(--color-chart-2)",
  "var(--color-chart-3)",
  "var(--color-chart-4)",
  "var(--color-chart-5)",
  "var(--color-chart-6)",
  "var(--color-chart-7)",
  "var(--color-chart-8)",
];

function getGenColor(gen: number): string {
  return GEN_COLORS[gen % GEN_COLORS.length] ?? "var(--color-text-muted)";
}

export default function PopulationTracker({
  populationStats: stats,
  agents,
  tickHistory,
}: PopulationTrackerProps) {

  // Compute generation distribution.
  const generationDist = useMemo(() => {
    const dist: Record<number, number> = {};
    for (const agent of agents) {
      if (agent.alive) {
        dist[agent.generation] = (dist[agent.generation] ?? 0) + 1;
      }
    }
    return Object.entries(dist)
      .map(([gen, count]) => ({ generation: Number(gen), count }))
      .sort((a, b) => a.generation - b.generation);
  }, [agents]);

  // Find oldest alive agent.
  const oldestAgent = useMemo(() => {
    let oldest: AgentListItem | null = null;
    for (const agent of agents) {
      if (agent.alive && agent.vitals) {
        if (!oldest || !oldest.vitals || agent.vitals.age > oldest.vitals.age) {
          oldest = agent;
        }
      }
    }
    return oldest;
  }, [agents]);

  // Age distribution histogram data.
  const ageDistribution = useMemo(() => {
    const ages = agents.filter((a) => a.alive && a.vitals).map((a) => a.vitals?.age ?? 0);

    if (ages.length === 0) return [];

    const maxAge = Math.max(...ages);
    const bucketSize = Math.max(50, Math.ceil(maxAge / 8));
    const buckets: { range: string; count: number }[] = [];

    for (let i = 0; i <= maxAge; i += bucketSize) {
      const count = ages.filter((a) => a >= i && a < i + bucketSize).length;
      buckets.push({ range: `${i}-${i + bucketSize}`, count });
    }

    return buckets;
  }, [agents]);

  // Sex ratio computation.
  const sexRatio = useMemo(() => {
    let male = 0;
    let female = 0;
    for (const agent of agents) {
      if (agent.alive) {
        if (agent.sex === "Male") male++;
        else if (agent.sex === "Female") female++;
      }
    }
    return { male, female };
  }, [agents]);

  // Death cause breakdown.
  const deathBreakdown = useMemo(() => {
    const causes: Record<string, number> = {};
    for (const agent of agents) {
      if (!agent.alive && agent.cause_of_death) {
        const cause = agent.cause_of_death;
        causes[cause] = (causes[cause] ?? 0) + 1;
      }
    }
    return Object.entries(causes)
      .map(([cause, count]) => ({ cause, count }))
      .sort((a, b) => b.count - a.count);
  }, [agents]);

  // Death table: all dead agents with details.
  const deathTable = useMemo(() => {
    return agents
      .filter((a) => !a.alive && a.died_at_tick !== null)
      .map((a) => ({
        name: a.name,
        sex: a.sex,
        age: a.died_at_tick !== null ? a.died_at_tick - a.born_at_tick : 0,
        cause: a.cause_of_death ?? "Unknown",
        tick: a.died_at_tick ?? 0,
        generation: a.generation,
      }))
      .sort((a, b) => b.tick - a.tick);
  }, [agents]);

  // Lifespan distribution by generation (for dead agents only).
  const lifespanByGeneration = useMemo(() => {
    const deadAgents = agents.filter((a) => !a.alive && a.died_at_tick !== null);
    if (deadAgents.length === 0) return { buckets: [] as { range: string; rangeStart: number; counts: Record<number, number> }[], generations: [] as number[] };

    const lifespans = deadAgents.map((a) => ({
      lifespan: (a.died_at_tick ?? 0) - a.born_at_tick,
      generation: a.generation,
    }));

    const maxLifespan = Math.max(...lifespans.map((l) => l.lifespan));
    const bucketSize = Math.max(25, Math.ceil(maxLifespan / 8));
    const generations = [...new Set(lifespans.map((l) => l.generation))].sort((a, b) => a - b);

    const buckets: { range: string; rangeStart: number; counts: Record<number, number> }[] = [];
    for (let i = 0; i <= maxLifespan; i += bucketSize) {
      const counts: Record<number, number> = {};
      for (const gen of generations) {
        counts[gen] = lifespans.filter(
          (l) => l.generation === gen && l.lifespan >= i && l.lifespan < i + bucketSize,
        ).length;
      }
      buckets.push({ range: `${i}-${i + bucketSize}`, rangeStart: i, counts });
    }

    return { buckets, generations };
  }, [agents]);

  if (!stats) {
    return (
      <div className="h-full bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
        <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
          Population Tracker
        </div>
        <div className="p-md">
          <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
            Waiting for population data...
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        Population Tracker
      </div>
      <div className="p-md flex-1 overflow-y-auto">
        {/* Top stats */}
        <div className="flex gap-sm mb-md flex-wrap">
          <div className="flex-1 min-w-[80px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Alive
            </div>
            <div className="text-lg font-bold text-text-accent font-mono">
              {formatNumber(stats.total_alive)}
            </div>
          </div>
          <div className="flex-1 min-w-[80px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Dead
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatNumber(stats.total_dead)}
            </div>
          </div>
          <div className="flex-1 min-w-[80px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Avg Age
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatDecimal(stats.average_age, 0)}
            </div>
          </div>
          <div className="flex-1 min-w-[80px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Births
            </div>
            <div className="text-lg font-bold text-success font-mono">{stats.births_this_tick}</div>
          </div>
          <div className="flex-1 min-w-[80px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Deaths
            </div>
            <div className="text-lg font-bold text-danger font-mono">{stats.deaths_this_tick}</div>
          </div>
          <div className="flex-1 min-w-[80px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Sex Ratio
            </div>
            <div className="text-sm font-bold text-text-primary font-mono">
              <span className="text-chart-1">{sexRatio.male}</span>
              <span className="text-text-muted mx-0.5">/</span>
              <span className="text-chart-5">{sexRatio.female}</span>
            </div>
          </div>
        </div>

        {/* Oldest agent highlight */}
        {oldestAgent && oldestAgent.vitals && (
          <div className="flex items-center gap-sm px-md py-sm bg-bg-tertiary rounded-sm mb-md text-sm font-mono">
            <span className="text-text-secondary">Oldest:</span>
            <span className="text-text-accent font-semibold">
              {oldestAgent.sex === "Male" ? "\u2642" : "\u2640"} {oldestAgent.name}
            </span>
            <span className="text-text-muted">
              (age {formatNumber(oldestAgent.vitals.age)} ticks, Gen {oldestAgent.generation})
            </span>
          </div>
        )}

        {/* Population over time chart */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Population Over Time
        </div>
        <PopulationChart tickHistory={tickHistory} />

        {/* Generation distribution */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Generation Distribution
        </div>
        <div className="flex gap-sm mb-md flex-wrap">
          {generationDist.map((g) => (
            <div
              key={g.generation}
              className="flex-1 min-w-[70px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center"
            >
              <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
                Gen {g.generation}
              </div>
              <div className="text-lg font-bold text-text-primary font-mono">{g.count}</div>
            </div>
          ))}
          {generationDist.length === 0 && (
            <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
              <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
                Gen 0
              </div>
              <div className="text-lg font-bold text-text-primary font-mono">0</div>
            </div>
          )}
        </div>

        {/* Age distribution histogram */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Age Distribution
        </div>
        <AgeHistogram data={ageDistribution} />

        {/* Death cause breakdown */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Cause of Death Breakdown
        </div>
        {deathBreakdown.length > 0 ? (
          <div className="flex gap-lg mb-md items-start flex-wrap">
            <DeathPieChart data={deathBreakdown} />
            <div className="flex-1 min-w-[200px]">
              <div className="flex flex-wrap gap-sm">
                {deathBreakdown.map((d) => (
                  <div key={d.cause} className="flex items-center gap-1.5 text-xs font-mono">
                    <span
                      className="w-2.5 h-2.5 rounded-sm inline-block shrink-0"
                      style={{ background: getDeathCauseColor(d.cause) }}
                    />
                    <span className="text-text-primary">{formatCauseName(d.cause)}</span>
                    <span className="text-text-muted">({d.count})</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="text-text-muted font-mono text-xs mb-md">No deaths recorded yet.</div>
        )}

        {/* Death table */}
        {deathTable.length > 0 && (
          <DeathTable deaths={deathTable} />
        )}

        {/* Lifespan distribution by generation */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Lifespan Distribution by Generation
        </div>
        {lifespanByGeneration.buckets.length > 0 ? (
          <>
            <LifespanHistogram data={lifespanByGeneration} />
            <div className="flex gap-md mt-sm mb-md flex-wrap">
              {lifespanByGeneration.generations.map((gen) => (
                <div key={gen} className="flex items-center gap-1.5 text-xs font-mono">
                  <span
                    className="w-2.5 h-2.5 rounded-sm inline-block shrink-0"
                    style={{ background: getGenColor(gen) }}
                  />
                  <span className="text-text-secondary">Gen {gen}</span>
                </div>
              ))}
            </div>
          </>
        ) : (
          <div className="text-text-muted font-mono text-xs mb-md">No lifespan data yet (no deaths recorded).</div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Population Over Time Chart
// ---------------------------------------------------------------------------

function PopulationChart({ tickHistory }: { tickHistory: TickBroadcast[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  const sortedHistory = useMemo(
    () => [...tickHistory].sort((a, b) => a.tick - b.tick),
    [tickHistory],
  );

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (sortedHistory.length < 2) {
      svg
        .attr("viewBox", "0 0 500 140")
        .append("text")
        .attr("x", 250)
        .attr("y", 70)
        .attr("text-anchor", "middle")
        .attr("fill", "var(--color-text-muted)")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("Waiting for tick data...");
      return;
    }

    const width = 500;
    const height = 140;
    const margin = { top: 10, right: 10, bottom: 25, left: 35 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const x = d3
      .scaleLinear()
      .domain(d3.extent(sortedHistory, (d) => d.tick) as [number, number])
      .range([0, innerW]);

    const yMax = d3.max(sortedHistory, (d) => d.agents_alive) ?? 1;
    const y = d3
      .scaleLinear()
      .domain([0, yMax + 2])
      .range([innerH, 0]);

    // Axes.
    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(
        d3
          .axisBottom(x)
          .ticks(6)
          .tickFormat((d) => `T${d}`),
      );

    g.append("g").attr("class", "axis").call(d3.axisLeft(y).ticks(5));

    // Area.
    const area = d3
      .area<TickBroadcast>()
      .x((d) => x(d.tick))
      .y0(innerH)
      .y1((d) => y(d.agents_alive))
      .curve(d3.curveMonotoneX);

    g.append("path")
      .datum(sortedHistory)
      .attr("fill", "var(--color-chart-2)")
      .attr("opacity", 0.15)
      .attr("d", area);

    // Line.
    const line = d3
      .line<TickBroadcast>()
      .x((d) => x(d.tick))
      .y((d) => y(d.agents_alive))
      .curve(d3.curveMonotoneX);

    g.append("path")
      .datum(sortedHistory)
      .attr("fill", "none")
      .attr("stroke", "var(--color-chart-2)")
      .attr("stroke-width", 2)
      .attr("d", line);

    // Death markers.
    const deathTicks = sortedHistory.filter((t) => t.deaths_this_tick > 0);
    g.selectAll(".death-marker")
      .data(deathTicks)
      .join("circle")
      .attr("cx", (d) => x(d.tick))
      .attr("cy", (d) => y(d.agents_alive))
      .attr("r", 4)
      .attr("fill", "var(--color-danger)")
      .attr("stroke", "var(--color-bg-primary)")
      .attr("stroke-width", 1.5);
  }, [sortedHistory]);

  return (
    <div className="chart-container mb-md">
      <svg ref={svgRef} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Age Distribution Histogram
// ---------------------------------------------------------------------------

function AgeHistogram({ data }: { data: { range: string; count: number }[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (data.length === 0) {
      svg
        .attr("viewBox", "0 0 500 100")
        .append("text")
        .attr("x", 250)
        .attr("y", 50)
        .attr("text-anchor", "middle")
        .attr("fill", "var(--color-text-muted)")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("No age data available");
      return;
    }

    const width = 500;
    const height = 100;
    const margin = { top: 5, right: 10, bottom: 25, left: 30 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const x = d3
      .scaleBand()
      .domain(data.map((d) => d.range))
      .range([0, innerW])
      .padding(0.2);

    const y = d3
      .scaleLinear()
      .domain([0, d3.max(data, (d) => d.count) ?? 1])
      .range([innerH, 0]);

    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(d3.axisBottom(x))
      .selectAll("text")
      .attr("font-size", "8px");

    g.append("g").attr("class", "axis").call(d3.axisLeft(y).ticks(3));

    g.selectAll("rect")
      .data(data)
      .join("rect")
      .attr("x", (d) => x(d.range) ?? 0)
      .attr("y", (d) => y(d.count))
      .attr("width", x.bandwidth())
      .attr("height", (d) => innerH - y(d.count))
      .attr("fill", "var(--color-chart-4)")
      .attr("opacity", 0.7);
  }, [data]);

  return (
    <div className="chart-container mb-md">
      <svg ref={svgRef} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Death Pie Chart (SVG arc)
// ---------------------------------------------------------------------------

function DeathPieChart({ data }: { data: { cause: string; count: number }[] }) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    const size = 160;
    const radius = size / 2 - 10;

    svg.attr("viewBox", `0 0 ${size} ${size}`);
    const g = svg.append("g").attr("transform", `translate(${size / 2},${size / 2})`);

    const pie = d3
      .pie<{ cause: string; count: number }>()
      .value((d) => d.count)
      .sort(null);

    const arc = d3
      .arc<d3.PieArcDatum<{ cause: string; count: number }>>()
      .innerRadius(radius * 0.45)
      .outerRadius(radius);

    const arcs = pie(data);
    const total = d3.sum(data, (d) => d.count);

    g.selectAll("path")
      .data(arcs)
      .join("path")
      .attr("d", arc)
      .attr("fill", (d) => getDeathCauseColor(d.data.cause))
      .attr("stroke", "var(--color-bg-secondary)")
      .attr("stroke-width", 2);

    // Center label: total deaths.
    g.append("text")
      .attr("text-anchor", "middle")
      .attr("dominant-baseline", "central")
      .attr("fill", "var(--color-text-primary)")
      .attr("font-family", "var(--font-mono)")
      .attr("font-size", "14px")
      .attr("font-weight", "bold")
      .text(String(total));

  }, [data]);

  return (
    <div className="w-[160px] h-[160px] shrink-0">
      <svg ref={svgRef} className="w-full h-full" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Death Table (scrollable, most recent first)
// ---------------------------------------------------------------------------

interface DeathEntry {
  name: string;
  sex: "Male" | "Female" | undefined;
  age: number;
  cause: string;
  tick: number;
  generation: number;
}

function DeathTable({ deaths }: { deaths: DeathEntry[] }) {
  const [showAll, setShowAll] = useState(false);
  const display = showAll ? deaths : deaths.slice(0, 10);

  return (
    <div className="mb-md">
      <div className="text-xs font-mono">
        <div className="grid grid-cols-[1fr_30px_60px_90px_70px_50px] px-sm py-xs text-text-muted border-b border-border-primary">
          <span>Name</span>
          <span>Sex</span>
          <span className="text-right">Age</span>
          <span className="text-right">Cause</span>
          <span className="text-right">Tick</span>
          <span className="text-right">Gen</span>
        </div>
        {display.map((d, i) => (
          <div
            key={`${d.name}-${d.tick}-${i}`}
            className="grid grid-cols-[1fr_30px_60px_90px_70px_50px] px-sm py-xs border-b border-border-secondary"
          >
            <span className="text-text-primary truncate">{d.name}</span>
            <span className="text-text-secondary">{d.sex === "Male" ? "\u2642" : "\u2640"}</span>
            <span className="text-right text-text-primary">{formatNumber(d.age)}</span>
            <span className="text-right text-text-secondary truncate">{formatCauseName(d.cause)}</span>
            <span className="text-right text-text-muted">{formatTick(d.tick)}</span>
            <span className="text-right text-text-muted">{d.generation}</span>
          </div>
        ))}
      </div>
      {deaths.length > 10 && (
        <button
          className="mt-sm text-xs font-mono text-text-accent cursor-pointer bg-transparent border-0 hover:underline"
          onClick={() => setShowAll(!showAll)}
        >
          {showAll ? "Show less" : `Show all ${deaths.length} deaths`}
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Lifespan Distribution Histogram by Generation
// ---------------------------------------------------------------------------

function LifespanHistogram({
  data,
}: {
  data: { buckets: { range: string; rangeStart: number; counts: Record<number, number> }[]; generations: number[] };
}) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    const { buckets, generations } = data;
    if (buckets.length === 0) return;

    const width = 500;
    const height = 130;
    const margin = { top: 8, right: 10, bottom: 28, left: 35 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const x = d3
      .scaleBand()
      .domain(buckets.map((b) => b.range))
      .range([0, innerW])
      .padding(0.2);

    // For stacked bars, compute max total per bucket.
    const maxTotal = d3.max(buckets, (b) => {
      return d3.sum(generations, (gen) => b.counts[gen] ?? 0);
    }) ?? 1;

    const y = d3.scaleLinear().domain([0, maxTotal]).range([innerH, 0]);

    // X axis.
    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(d3.axisBottom(x))
      .selectAll("text")
      .attr("font-size", "8px")
      .attr("transform", "rotate(-20)")
      .attr("text-anchor", "end");

    // Y axis.
    g.append("g").attr("class", "axis").call(d3.axisLeft(y).ticks(4));

    // Stacked bars.
    buckets.forEach((bucket) => {
      const xPos = x(bucket.range) ?? 0;
      const barWidth = x.bandwidth();
      let yOffset = innerH;

      for (const gen of generations) {
        const count = bucket.counts[gen] ?? 0;
        if (count === 0) continue;
        const barHeight = innerH - y(count);
        yOffset -= barHeight;

        g.append("rect")
          .attr("x", xPos)
          .attr("y", yOffset)
          .attr("width", barWidth)
          .attr("height", barHeight)
          .attr("fill", getGenColor(gen))
          .attr("opacity", 0.75);
      }
    });

    // Y-axis label.
    g.append("text")
      .attr("transform", "rotate(-90)")
      .attr("x", -innerH / 2)
      .attr("y", -28)
      .attr("text-anchor", "middle")
      .attr("fill", "var(--color-text-muted)")
      .attr("font-size", "9px")
      .attr("font-family", "var(--font-mono)")
      .text("Deaths");

  }, [data]);

  return (
    <div className="chart-container mb-sm">
      <svg ref={svgRef} />
    </div>
  );
}

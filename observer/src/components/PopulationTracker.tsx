/**
 * Population Tracker Panel (Task 4.5.6)
 *
 * Line chart of population over time. Average age display.
 * Generation distribution. Death causes breakdown.
 * Oldest living agent highlight.
 */
import { useEffect, useMemo, useRef } from "react";

import * as d3 from "d3";

import type { AgentListItem, PopulationStats, TickBroadcast } from "../types/generated/index.ts";
import { formatDecimal, formatNumber } from "../utils/format.ts";
import { MOCK_AGENTS, MOCK_WORLD_SNAPSHOT, generateMockTickHistory } from "../utils/mockData.ts";

interface PopulationTrackerProps {
  populationStats: PopulationStats | null;
  agents: AgentListItem[];
  tickHistory: TickBroadcast[];
  useMock?: boolean;
}

export default function PopulationTracker({
  populationStats: propStats,
  agents: propAgents,
  tickHistory: propHistory,
  useMock = false,
}: PopulationTrackerProps) {
  const stats = useMock ? MOCK_WORLD_SNAPSHOT.population : propStats;
  const agents = useMock ? MOCK_AGENTS : propAgents;
  const tickHistory = useMock ? generateMockTickHistory(100) : propHistory;

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
        <div className="flex gap-sm mb-md">
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Alive
            </div>
            <div className="text-lg font-bold text-text-accent font-mono">
              {formatNumber(stats.total_alive)}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Dead
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatNumber(stats.total_dead)}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Avg Age
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatDecimal(stats.average_age, 0)}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Births
            </div>
            <div className="text-lg font-bold text-success font-mono">{stats.births_this_tick}</div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Deaths
            </div>
            <div className="text-lg font-bold text-danger font-mono">{stats.deaths_this_tick}</div>
          </div>
        </div>

        {/* Oldest agent highlight */}
        {oldestAgent && oldestAgent.vitals && (
          <div className="flex items-center gap-sm px-md py-sm bg-bg-tertiary rounded-sm mb-md text-sm font-mono">
            <span className="text-text-secondary">Oldest:</span>
            <span className="text-text-accent font-semibold">{oldestAgent.name}</span>
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
        <div className="flex gap-sm mb-md">
          {generationDist.map((g) => (
            <div
              key={g.generation}
              className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center"
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
        .attr("fill", "#484f58")
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
        .attr("fill", "#484f58")
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
    <div className="chart-container">
      <svg ref={svgRef} />
    </div>
  );
}

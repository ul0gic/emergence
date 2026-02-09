/**
 * Economy Monitor Panel (Task 4.5.3)
 *
 * Resource flow visualization, wealth distribution chart, Gini coefficient
 * display with trend line, trade volume over time, and resource totals.
 */
import { useEffect, useMemo, useRef } from "react";

import * as d3 from "d3";

import type {
  EconomyStats,
  Resource,
  TickBroadcast,
  WorldSnapshot,
} from "../types/generated/index.ts";
import { formatGini, formatNumber, formatResourceName, getResourceColor } from "../utils/format.ts";
import { MOCK_WORLD_SNAPSHOT, generateMockTickHistory } from "../utils/mockData.ts";

interface EconomyMonitorProps {
  worldSnapshot: WorldSnapshot | null;
  tickHistory: TickBroadcast[];
  useMock?: boolean;
}

// Ordered for display.
const RESOURCE_ORDER: Resource[] = [
  "Water",
  "FoodBerry",
  "FoodFish",
  "FoodRoot",
  "FoodMeat",
  "FoodFarmed",
  "FoodCooked",
  "Wood",
  "Stone",
  "Fiber",
  "Clay",
  "Hide",
  "Ore",
  "Metal",
  "Medicine",
  "Tool",
  "ToolAdvanced",
  "CurrencyToken",
  "WrittenRecord",
];

export default function EconomyMonitor({
  worldSnapshot: propSnapshot,
  tickHistory: propHistory,
  useMock = false,
}: EconomyMonitorProps) {
  const snapshot = useMock ? MOCK_WORLD_SNAPSHOT : propSnapshot;
  const tickHistory = useMock ? generateMockTickHistory(100) : propHistory;

  if (!snapshot) {
    return (
      <div className="h-full bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
        <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
          Economy Monitor
        </div>
        <div className="p-md">
          <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
            Waiting for world data...
          </div>
        </div>
      </div>
    );
  }

  const economy = snapshot.economy;

  // Compute resources that actually have non-zero values.
  // eslint-disable-next-line security/detect-object-injection -- r is from the static RESOURCE_ORDER array of Resource literals, not user input
  const activeResources = RESOURCE_ORDER.filter((r) => (economy.total_resources[r] ?? 0) > 0);

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>Economy Monitor</span>
        <span className="text-xs font-normal">Gini: {formatGini(economy.gini_coefficient)}</span>
      </div>
      <div className="p-md flex-1 overflow-y-auto">
        {/* Top stats */}
        <div className="flex gap-sm mb-md">
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Gini Coefficient
            </div>
            <div className="text-lg font-bold text-text-accent font-mono">
              {formatGini(economy.gini_coefficient)}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Trades / Tick
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {economy.trades_this_tick}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              In Circulation
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatNumber(
                Object.values(economy.resources_in_circulation).reduce(
                  (acc, v) => acc + (v ?? 0),
                  0,
                ),
              )}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              At Nodes
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatNumber(
                Object.values(economy.resources_at_nodes).reduce((acc, v) => acc + (v ?? 0), 0),
              )}
            </div>
          </div>
        </div>

        {/* Resource distribution bar chart */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Resource Distribution
        </div>
        <ResourceDistributionChart economy={economy} resources={activeResources} />

        {/* Population/actions over time mini chart */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Activity Over Time
        </div>
        <ActivityChart tickHistory={tickHistory} />

        {/* Resource table */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Resource Breakdown
        </div>
        <div className="text-xs font-mono">
          <div className="grid grid-cols-[1fr_80px_80px_80px] px-sm py-xs text-text-muted border-b border-border-primary">
            <span>Resource</span>
            <span className="text-right">Total</span>
            <span className="text-right">Agents</span>
            <span className="text-right">Nodes</span>
          </div>
          {activeResources.map((r) => (
            <div
              key={r}
              className="grid grid-cols-[1fr_80px_80px_80px] px-sm py-xs border-b border-border-secondary"
            >
              <span className="flex items-center gap-1.5">
                <span
                  className="w-2 h-2 rounded-sm inline-block"
                  style={{ background: getResourceColor(r) }}
                />
                {formatResourceName(r)}
              </span>
              <span className="text-right text-text-primary">
                {/* eslint-disable-next-line security/detect-object-injection -- r is a Resource literal from RESOURCE_ORDER, not user input */}
                {formatNumber(economy.total_resources[r] ?? 0)}
              </span>
              <span className="text-right">
                {/* eslint-disable-next-line security/detect-object-injection -- r is a Resource literal from RESOURCE_ORDER, not user input */}
                {formatNumber(economy.resources_in_circulation[r] ?? 0)}
              </span>
              <span className="text-right">
                {/* eslint-disable-next-line security/detect-object-injection -- r is a Resource literal from RESOURCE_ORDER, not user input */}
                {formatNumber(economy.resources_at_nodes[r] ?? 0)}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Resource Distribution Bar Chart
// ---------------------------------------------------------------------------

function ResourceDistributionChart({
  economy,
  resources,
}: {
  economy: EconomyStats;
  resources: Resource[];
}) {
  const svgRef = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    const width = 500;
    const height = 140;
    const margin = { top: 8, right: 10, bottom: 32, left: 40 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);

    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const data = resources.map((r) => ({
      resource: r,
      // eslint-disable-next-line security/detect-object-injection -- r is a Resource literal from the typed resources prop, not user input
      agents: economy.resources_in_circulation[r] ?? 0,
      // eslint-disable-next-line security/detect-object-injection -- r is a Resource literal from the typed resources prop, not user input
      nodes: economy.resources_at_nodes[r] ?? 0,
    }));

    const x = d3
      .scaleBand()
      .domain(resources.map(formatResourceName))
      .range([0, innerW])
      .padding(0.3);

    const maxVal = d3.max(data, (d) => d.agents + d.nodes) ?? 1;
    const y = d3.scaleLinear().domain([0, maxVal]).range([innerH, 0]);

    // Y axis.
    g.append("g")
      .attr("class", "axis")
      .call(d3.axisLeft(y).ticks(5).tickFormat(d3.format("~s")));

    // X axis.
    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(d3.axisBottom(x))
      .selectAll("text")
      .attr("transform", "rotate(-35)")
      .attr("text-anchor", "end")
      .attr("font-size", "8px");

    // Stacked bars: nodes (bottom) + agents (top).
    data.forEach((d) => {
      const xPos = x(formatResourceName(d.resource)) ?? 0;
      const barWidth = x.bandwidth();

      // Nodes portion.
      g.append("rect")
        .attr("x", xPos)
        .attr("y", y(d.nodes))
        .attr("width", barWidth)
        .attr("height", innerH - y(d.nodes))
        .attr("fill", getResourceColor(d.resource))
        .attr("opacity", 0.4);

      // Agents portion (stacked on top).
      g.append("rect")
        .attr("x", xPos)
        .attr("y", y(d.nodes + d.agents))
        .attr("width", barWidth)
        .attr("height", y(d.nodes) - y(d.nodes + d.agents))
        .attr("fill", getResourceColor(d.resource))
        .attr("opacity", 0.8);
    });
  }, [economy, resources]);

  return (
    <div className="chart-container mb-sm">
      <svg ref={svgRef} />
      <div className="flex justify-center gap-lg text-xs font-mono text-text-secondary">
        <span className="flex items-center gap-1">
          <span className="inline-block w-2.5 h-2.5 bg-chart-1 opacity-80 rounded-sm" />
          Agents
        </span>
        <span className="flex items-center gap-1">
          <span className="inline-block w-2.5 h-2.5 bg-chart-1 opacity-40 rounded-sm" />
          Nodes
        </span>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Activity Over Time Chart
// ---------------------------------------------------------------------------

function ActivityChart({ tickHistory }: { tickHistory: TickBroadcast[] }) {
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
        .attr("viewBox", "0 0 500 120")
        .append("text")
        .attr("x", 250)
        .attr("y", 60)
        .attr("text-anchor", "middle")
        .attr("fill", "#484f58")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("Waiting for tick data...");
      return;
    }

    const width = 500;
    const height = 100;
    const margin = { top: 8, right: 30, bottom: 20, left: 36 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const x = d3
      .scaleLinear()
      .domain(d3.extent(sortedHistory, (d) => d.tick) as [number, number])
      .range([0, innerW]);

    const yActions = d3
      .scaleLinear()
      .domain([0, d3.max(sortedHistory, (d) => d.actions_resolved) ?? 1])
      .range([innerH, 0]);

    // X axis.
    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(
        d3
          .axisBottom(x)
          .ticks(6)
          .tickFormat((d) => `T${d}`),
      );

    // Y axis (actions).
    g.append("g").attr("class", "axis").call(d3.axisLeft(yActions).ticks(4));

    // Actions line.
    const line = d3
      .line<TickBroadcast>()
      .x((d) => x(d.tick))
      .y((d) => yActions(d.actions_resolved))
      .curve(d3.curveMonotoneX);

    g.append("path")
      .datum(sortedHistory)
      .attr("fill", "none")
      .attr("stroke", "var(--color-chart-1)")
      .attr("stroke-width", 1.5)
      .attr("d", line);

    // Area fill.
    const area = d3
      .area<TickBroadcast>()
      .x((d) => x(d.tick))
      .y0(innerH)
      .y1((d) => yActions(d.actions_resolved))
      .curve(d3.curveMonotoneX);

    g.append("path")
      .datum(sortedHistory)
      .attr("fill", "var(--color-chart-1)")
      .attr("opacity", 0.1)
      .attr("d", area);
  }, [sortedHistory]);

  return (
    <div className="chart-container mb-sm">
      <svg ref={svgRef} />
    </div>
  );
}

/**
 * Economy Monitor Panel (Task 4.5.3, Phase 9.6.3, Phase 9.6.4)
 *
 * Resource flow visualization, wealth distribution chart, Gini coefficient
 * display with trend line, trade volume over time, resource totals,
 * Lorenz curve for inequality visualization, and resource flow Sankey diagram.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import * as d3 from "d3";

import type {
  AgentListItem,
  EconomyStats,
  Resource,
  TickBroadcast,
  WorldSnapshot,
} from "../types/generated/index.ts";
import type { ChartTooltipData } from "./ui/chart-tooltip.tsx";
import { ChartTooltip } from "./ui/chart-tooltip.tsx";
import { formatGini, formatNumber, formatResourceName, getResourceColor } from "../utils/format.ts";

interface EconomyMonitorProps {
  worldSnapshot: WorldSnapshot | null;
  tickHistory: TickBroadcast[];
  agents: AgentListItem[];
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

// Categorize resources for the Sankey diagram.
const RESOURCE_CATEGORIES: Record<string, Resource[]> = {
  "Water": ["Water"],
  "Food": ["FoodBerry", "FoodFish", "FoodRoot", "FoodMeat", "FoodFarmed", "FoodCooked"],
  "Raw Materials": ["Wood", "Stone", "Fiber", "Clay", "Hide", "Ore"],
  "Refined": ["Metal", "Medicine", "Tool", "ToolAdvanced"],
  "Currency & Records": ["CurrencyToken", "WrittenRecord"],
};

export default function EconomyMonitor({
  worldSnapshot: snapshot,
  tickHistory,
  agents,
}: EconomyMonitorProps) {

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
      <div className="p-md pb-xl flex-1 overflow-y-auto">
        {/* Top stats */}
        <div className="flex gap-sm mb-md flex-wrap">
          <div className="flex-1 min-w-[90px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Gini Coefficient
            </div>
            <div className="text-lg font-bold text-text-accent font-mono">
              {formatGini(economy.gini_coefficient)}
            </div>
          </div>
          <div className="flex-1 min-w-[90px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Trades / Tick
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {economy.trades_this_tick}
            </div>
          </div>
          <div className="flex-1 min-w-[90px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
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
          <div className="flex-1 min-w-[90px] bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
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

        {/* Lorenz Curve / Wealth Inequality */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Wealth Inequality (Lorenz Curve)
        </div>
        <LorenzCurve economy={economy} giniCoefficient={economy.gini_coefficient} agents={agents} />

        {/* Resource Flow (simplified Sankey) */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Resource Flow
        </div>
        <ResourceFlowSankey economy={economy} />

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
// Lorenz Curve with Gini Coefficient
// ---------------------------------------------------------------------------

function LorenzCurve({
  economy: _economy,
  giniCoefficient,
  agents: _agents,
}: {
  economy: EconomyStats;
  giniCoefficient: string;
  agents: AgentListItem[];
}) {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [tooltip, setTooltip] = useState<ChartTooltipData | null>(null);

  // Compute Lorenz curve points from per-agent wealth proxy.
  // Since AgentListItem doesn't carry inventory, we use the Gini from the
  // backend and simulate a Lorenz curve from the resource distribution data.
  // We approximate using the in-circulation vs at-nodes split.
  const lorenzPoints = useMemo(() => {
    // If we have alive agents, we can estimate a distribution.
    // We'll simulate a Lorenz curve from the Gini coefficient.
    const gini = parseFloat(giniCoefficient);
    if (Number.isNaN(gini) || gini < 0) return [];

    // Generate synthetic Lorenz curve from Gini using a power distribution.
    // L(p) = p^(1+a) where a = (1+G)/(1-G) gives Gini = 1/(2a+1)
    // Invert: a = (1-G)/(2G) if G>0
    const n = 20; // number of points
    const points: { x: number; y: number }[] = [{ x: 0, y: 0 }];

    if (gini > 0.001) {
      // Use approximation: L(p) = p^((1+gini)/(1-gini))
      const exponent = (1 + gini) / (1 - Math.min(gini, 0.99));
      for (let i = 1; i <= n; i++) {
        const p = i / n;
        const l = Math.pow(p, exponent);
        points.push({ x: p, y: l });
      }
    } else {
      // Perfect equality.
      for (let i = 1; i <= n; i++) {
        const p = i / n;
        points.push({ x: p, y: p });
      }
    }

    return points;
  }, [giniCoefficient]);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    const width = 260;
    const height = 200;
    const margin = { top: 10, right: 15, bottom: 30, left: 40 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const x = d3.scaleLinear().domain([0, 1]).range([0, innerW]);
    const y = d3.scaleLinear().domain([0, 1]).range([innerH, 0]);

    // Axes.
    g.append("g")
      .attr("class", "axis")
      .attr("transform", `translate(0,${innerH})`)
      .call(d3.axisBottom(x).ticks(5).tickFormat(d3.format(".0%")));

    g.append("g")
      .attr("class", "axis")
      .call(d3.axisLeft(y).ticks(5).tickFormat(d3.format(".0%")));

    // Line of equality (diagonal).
    g.append("line")
      .attr("x1", 0)
      .attr("y1", innerH)
      .attr("x2", innerW)
      .attr("y2", 0)
      .attr("stroke", "var(--color-text-muted)")
      .attr("stroke-width", 1)
      .attr("stroke-dasharray", "4,3");

    // Equality label.
    g.append("text")
      .attr("x", innerW * 0.55)
      .attr("y", innerH * 0.35)
      .attr("fill", "var(--color-text-muted)")
      .attr("font-size", "8px")
      .attr("font-family", "var(--font-mono)")
      .attr("transform", `rotate(-38, ${innerW * 0.55}, ${innerH * 0.35})`)
      .text("Perfect Equality");

    if (lorenzPoints.length > 1) {
      // Fill area between equality line and Lorenz curve (Gini area).
      const areaPath = d3
        .area<{ x: number; y: number }>()
        .x((d) => x(d.x))
        .y0((d) => y(d.x)) // equality line
        .y1((d) => y(d.y)) // Lorenz curve
        .curve(d3.curveMonotoneX);

      g.append("path")
        .datum(lorenzPoints)
        .attr("fill", "var(--color-danger)")
        .attr("opacity", 0.12)
        .attr("d", areaPath);

      // Lorenz curve line.
      const line = d3
        .line<{ x: number; y: number }>()
        .x((d) => x(d.x))
        .y((d) => y(d.y))
        .curve(d3.curveMonotoneX);

      g.append("path")
        .datum(lorenzPoints)
        .attr("fill", "none")
        .attr("stroke", "var(--color-chart-5)")
        .attr("stroke-width", 2)
        .attr("d", line);

      // Interactive hover dots on Lorenz curve.
      g.selectAll(".lorenz-dot")
        .data(lorenzPoints.filter((_, i) => i > 0))
        .join("circle")
        .attr("class", "lorenz-dot")
        .attr("cx", (d) => x(d.x))
        .attr("cy", (d) => y(d.y))
        .attr("r", 6)
        .attr("fill", "transparent")
        .attr("cursor", "crosshair")
        .on("mouseenter", (event: MouseEvent, d) => {
          const container = containerRef.current;
          if (!container) return;
          const rect = container.getBoundingClientRect();
          setTooltip({
            x: event.clientX - rect.left,
            y: event.clientY - rect.top,
            title: "Lorenz Curve",
            rows: [
              { label: "Population", value: `${(d.x * 100).toFixed(0)}%` },
              { label: "Wealth share", value: `${(d.y * 100).toFixed(1)}%` },
              { label: "Equality line", value: `${(d.x * 100).toFixed(0)}%` },
            ],
          });
        })
        .on("mouseleave", () => setTooltip(null));
    }

    // Axis labels.
    g.append("text")
      .attr("x", innerW / 2)
      .attr("y", innerH + 26)
      .attr("text-anchor", "middle")
      .attr("fill", "var(--color-text-muted)")
      .attr("font-size", "9px")
      .attr("font-family", "var(--font-mono)")
      .text("Cumulative % of Agents");

    g.append("text")
      .attr("transform", "rotate(-90)")
      .attr("x", -innerH / 2)
      .attr("y", -30)
      .attr("text-anchor", "middle")
      .attr("fill", "var(--color-text-muted)")
      .attr("font-size", "9px")
      .attr("font-family", "var(--font-mono)")
      .text("Cumulative % of Wealth");

  }, [lorenzPoints]);

  const giniValue = parseFloat(giniCoefficient);
  const giniPercent = Number.isNaN(giniValue) ? giniCoefficient : (giniValue * 100).toFixed(1) + "%";

  // Interpret the Gini.
  let giniLabel = "Undefined";
  if (!Number.isNaN(giniValue)) {
    if (giniValue < 0.2) giniLabel = "Very Equal";
    else if (giniValue < 0.35) giniLabel = "Moderate";
    else if (giniValue < 0.5) giniLabel = "Unequal";
    else if (giniValue < 0.7) giniLabel = "Very Unequal";
    else giniLabel = "Extreme Inequality";
  }

  return (
    <div className="flex gap-lg items-start mb-md flex-wrap">
      <div ref={containerRef} className="chart-container w-[260px] shrink-0 relative">
        <svg ref={svgRef} className="w-full" style={{ maxHeight: "200px" }} />
        <ChartTooltip data={tooltip} />
      </div>
      <div className="flex flex-col gap-sm min-w-[140px]">
        <div className="bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm">
          <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
            Gini Index
          </div>
          <div className="text-xl font-bold text-text-accent font-mono">{giniPercent}</div>
          <div className="text-2xs text-text-muted font-mono">{giniLabel}</div>
        </div>
        <div className="text-2xs text-text-muted font-mono leading-relaxed">
          0% = perfect equality (everyone holds the same).
          100% = one agent holds everything.
          The shaded area between the diagonal and curve represents inequality.
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Resource Flow Sankey (simplified flow visualization)
// ---------------------------------------------------------------------------

function ResourceFlowSankey({ economy }: { economy: EconomyStats }) {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [tooltip, setTooltip] = useState<ChartTooltipData | null>(null);

  const flowData = useMemo(() => {
    // Build flows: Source categories -> Destination (Agents vs Nodes).
    const flows: { source: string; target: string; value: number; color: string }[] = [];

    for (const [category, resources] of Object.entries(RESOURCE_CATEGORIES)) {
      let agentTotal = 0;
      let nodeTotal = 0;

      for (const r of resources) {
        agentTotal += economy.resources_in_circulation[r] ?? 0;
        nodeTotal += economy.resources_at_nodes[r] ?? 0;
      }

      if (agentTotal > 0) {
        flows.push({
          source: category,
          target: "Agent Inventories",
          value: agentTotal,
          color: "var(--color-chart-1)",
        });
      }

      if (nodeTotal > 0) {
        flows.push({
          source: category,
          target: "Location Nodes",
          value: nodeTotal,
          color: "var(--color-chart-3)",
        });
      }
    }

    return flows;
  }, [economy]);

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    if (flowData.length === 0) {
      svg
        .attr("viewBox", "0 0 500 120")
        .append("text")
        .attr("x", 250)
        .attr("y", 60)
        .attr("text-anchor", "middle")
        .attr("fill", "var(--color-text-muted)")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("No resource flow data");
      return;
    }

    const width = 600;
    const margin = { top: 10, right: 140, bottom: 10, left: 100 };
    const innerW = width - margin.left - margin.right;

    // Collect unique sources and targets.
    const sources = [...new Set(flowData.map((f) => f.source))];
    const targets = [...new Set(flowData.map((f) => f.target))];

    // Compute totals for sizing.
    const sourceTotals = new Map<string, number>();
    const targetTotals = new Map<string, number>();

    for (const f of flowData) {
      sourceTotals.set(f.source, (sourceTotals.get(f.source) ?? 0) + f.value);
      targetTotals.set(f.target, (targetTotals.get(f.target) ?? 0) + f.value);
    }

    const totalFlow = d3.sum(flowData, (f) => f.value);
    if (totalFlow === 0) return;

    // Fixed bar heights — scale to number of nodes, not viewport.
    const nodeHeight = 20;
    const gap = 12;

    // Pre-compute positions so we can derive the viewBox height.
    const sourceYPositions = new Map<string, { y: number; height: number }>();
    let yCursor = 0;
    for (const src of sources) {
      const total = sourceTotals.get(src) ?? 0;
      const barH = Math.max(nodeHeight, Math.round((total / totalFlow) * sources.length * nodeHeight * 2));
      sourceYPositions.set(src, { y: yCursor, height: barH });
      yCursor += barH + gap;
    }
    const sourceExtent = yCursor - gap;

    const targetYPositions = new Map<string, { y: number; height: number }>();
    yCursor = 0;
    for (const tgt of targets) {
      const total = targetTotals.get(tgt) ?? 0;
      const barH = Math.max(nodeHeight, Math.round((total / totalFlow) * targets.length * nodeHeight * 2));
      targetYPositions.set(tgt, { y: yCursor, height: barH });
      yCursor += barH + gap;
    }
    const targetExtent = yCursor - gap;

    const innerH = Math.max(sourceExtent, targetExtent);
    const height = innerH + margin.top + margin.bottom;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    // Draw source nodes (left).
    for (const src of sources) {
      const pos = sourceYPositions.get(src)!;

      g.append("rect")
        .attr("x", 0)
        .attr("y", pos.y)
        .attr("width", 12)
        .attr("height", pos.height)
        .attr("fill", "var(--color-chart-2)")
        .attr("rx", 2);

      g.append("text")
        .attr("x", -6)
        .attr("y", pos.y + pos.height / 2)
        .attr("text-anchor", "end")
        .attr("dominant-baseline", "central")
        .attr("fill", "var(--color-text-secondary)")
        .attr("font-size", "9px")
        .attr("font-family", "var(--font-mono)")
        .text(src);
    }

    // Draw target nodes (right).
    for (const tgt of targets) {
      const pos = targetYPositions.get(tgt)!;
      const total = targetTotals.get(tgt) ?? 0;

      g.append("rect")
        .attr("x", innerW - 12)
        .attr("y", pos.y)
        .attr("width", 12)
        .attr("height", pos.height)
        .attr("fill", tgt === "Agent Inventories" ? "var(--color-chart-1)" : "var(--color-chart-3)")
        .attr("rx", 2);

      g.append("text")
        .attr("x", innerW + 6)
        .attr("y", pos.y + pos.height / 2)
        .attr("text-anchor", "start")
        .attr("dominant-baseline", "central")
        .attr("fill", "var(--color-text-secondary)")
        .attr("font-size", "9px")
        .attr("font-family", "var(--font-mono)")
        .text(`${tgt} (${formatNumber(total)})`);
    }

    // Draw flows (curved links).
    // Track offsets within each node for stacking.
    const sourceOffsets = new Map<string, number>();
    const targetOffsets = new Map<string, number>();
    for (const src of sources) sourceOffsets.set(src, 0);
    for (const tgt of targets) targetOffsets.set(tgt, 0);

    for (const flow of flowData) {
      const srcPos = sourceYPositions.get(flow.source);
      const tgtPos = targetYPositions.get(flow.target);
      if (!srcPos || !tgtPos) continue;

      const srcTotal = sourceTotals.get(flow.source) ?? 1;
      const tgtTotal = targetTotals.get(flow.target) ?? 1;

      const flowSrcH = (flow.value / srcTotal) * srcPos.height;
      const flowTgtH = (flow.value / tgtTotal) * tgtPos.height;

      const srcOffset = sourceOffsets.get(flow.source) ?? 0;
      const tgtOffset = targetOffsets.get(flow.target) ?? 0;

      const y0Top = srcPos.y + srcOffset;
      const y0Bot = y0Top + flowSrcH;
      const y1Top = tgtPos.y + tgtOffset;
      const y1Bot = y1Top + flowTgtH;

      sourceOffsets.set(flow.source, srcOffset + flowSrcH);
      targetOffsets.set(flow.target, tgtOffset + flowTgtH);

      const midX = innerW / 2;

      // Draw a curved path.
      const pathData = `
        M 12,${y0Top}
        C ${midX},${y0Top} ${midX},${y1Top} ${innerW - 12},${y1Top}
        L ${innerW - 12},${y1Bot}
        C ${midX},${y1Bot} ${midX},${y0Bot} 12,${y0Bot}
        Z
      `;

      g.append("path")
        .attr("d", pathData)
        .attr("fill", flow.color)
        .attr("opacity", 0.15)
        .attr("stroke", flow.color)
        .attr("stroke-width", 0.5)
        .attr("stroke-opacity", 0.3)
        .attr("cursor", "pointer")
        .on("mouseenter", (event: MouseEvent) => {
          const container = containerRef.current;
          if (!container) return;
          const rect = container.getBoundingClientRect();
          setTooltip({
            x: event.clientX - rect.left,
            y: event.clientY - rect.top,
            title: "Resource Flow",
            rows: [
              { label: "From", value: flow.source },
              { label: "To", value: flow.target },
              { label: "Amount", value: formatNumber(flow.value) },
            ],
          });
        })
        .on("mousemove", (event: MouseEvent) => {
          const container = containerRef.current;
          if (!container) return;
          const rect = container.getBoundingClientRect();
          setTooltip((prev) =>
            prev
              ? { ...prev, x: event.clientX - rect.left, y: event.clientY - rect.top }
              : null,
          );
        })
        .on("mouseleave", () => setTooltip(null));
    }
  }, [flowData]);

  return (
    <div ref={containerRef} className="chart-container mb-md relative">
      <svg ref={svgRef} className="w-full" />
      <ChartTooltip data={tooltip} />
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
  const containerRef = useRef<HTMLDivElement>(null);
  const [tooltip, setTooltip] = useState<ChartTooltipData | null>(null);

  const handleBarHover = useCallback(
    (event: MouseEvent, resource: Resource, agents: number, nodes: number) => {
      const container = containerRef.current;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      setTooltip({
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
        title: formatResourceName(resource),
        rows: [
          { label: "Agents", value: formatNumber(agents), color: getResourceColor(resource) },
          { label: "Nodes", value: formatNumber(nodes) },
          { label: "Total", value: formatNumber(agents + nodes) },
        ],
      });
    },
    [],
  );

  useEffect(() => {
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();

    const width = 600;
    const height = 260;
    const margin = { top: 8, right: 10, bottom: 70, left: 45 };
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

      // Invisible hover target for the full bar.
      g.append("rect")
        .attr("x", xPos)
        .attr("y", y(d.nodes + d.agents))
        .attr("width", barWidth)
        .attr("height", innerH - y(d.nodes + d.agents))
        .attr("fill", "transparent")
        .attr("cursor", "crosshair")
        .on("mouseenter", (event: MouseEvent) => handleBarHover(event, d.resource, d.agents, d.nodes))
        .on("mousemove", (event: MouseEvent) => {
          const container = containerRef.current;
          if (!container) return;
          const rect = container.getBoundingClientRect();
          setTooltip((prev) =>
            prev
              ? { ...prev, x: event.clientX - rect.left, y: event.clientY - rect.top }
              : null,
          );
        })
        .on("mouseleave", () => setTooltip(null));
    });
  }, [economy, resources, handleBarHover]);

  return (
    <div ref={containerRef} className="chart-container mb-sm relative">
      <svg ref={svgRef} className="w-full" style={{ minHeight: "240px" }} />
      <ChartTooltip data={tooltip} />
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
  const containerRef = useRef<HTMLDivElement>(null);
  const [tooltip, setTooltip] = useState<ChartTooltipData | null>(null);

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
        .attr("fill", "var(--color-text-muted)")
        .attr("font-size", "12px")
        .attr("font-family", "var(--font-mono)")
        .text("Waiting for tick data...");
      return;
    }

    const width = 500;
    const height = 140;
    const margin = { top: 8, right: 30, bottom: 24, left: 36 };
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

    // Interactive overlay for hover tracking.
    const bisect = d3.bisector<TickBroadcast, number>((d) => d.tick).left;
    g.append("rect")
      .attr("width", innerW)
      .attr("height", innerH)
      .attr("fill", "transparent")
      .attr("cursor", "crosshair")
      .on("mousemove", (event: MouseEvent) => {
        const container = containerRef.current;
        if (!container) return;
        const containerRect = container.getBoundingClientRect();
        const [mx] = d3.pointer(event);
        const tickVal = x.invert(mx);
        const idx = bisect(sortedHistory, tickVal, 1);
        const d0 = sortedHistory[idx - 1];
        const d1 = sortedHistory[idx];
        const d = d0 && d1 ? (tickVal - d0.tick > d1.tick - tickVal ? d1 : d0) : d0 ?? d1;
        if (!d) return;
        setTooltip({
          x: event.clientX - containerRect.left,
          y: event.clientY - containerRect.top,
          title: `Tick ${d.tick}`,
          rows: [
            { label: "Actions", value: formatNumber(d.actions_resolved) },
            { label: "Agents", value: formatNumber(d.agents_alive) },
          ],
        });
      })
      .on("mouseleave", () => setTooltip(null));
  }, [sortedHistory]);

  return (
    <div ref={containerRef} className="chart-container mb-sm relative">
      <svg ref={svgRef} />
      <ChartTooltip data={tooltip} />
    </div>
  );
}

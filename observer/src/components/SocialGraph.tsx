/**
 * Social Network Graph (Task 4.5.4)
 *
 * D3.js force-directed graph of agent relationships. Edge color shows
 * positive (green) to negative (red). Edge thickness shows strength.
 * Node size shows number of connections. Click an edge for details.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import * as d3 from "d3";

import type { AgentListItem } from "../types/generated/index.ts";
import { formatDecimal } from "../utils/format.ts";
import { MOCK_AGENTS, MOCK_AGENT_DETAIL } from "../utils/mockData.ts";

interface SocialGraphProps {
  agents: AgentListItem[];
  /** Map of agent ID -> relationship map (agentId -> score string). */
  relationships: Map<string, Record<string, string | undefined>>;
  useMock?: boolean;
}

interface SocialNode extends d3.SimulationNodeDatum {
  id: string;
  name: string;
  alive: boolean;
  connectionCount: number;
}

interface SocialLink extends d3.SimulationLinkDatum<SocialNode> {
  score: number;
  sourceId: string;
  targetId: string;
}

function scoreToColor(score: number): string {
  if (score > 0.5) return "#3fb950";
  if (score > 0.2) return "#7ee787";
  if (score > -0.2) return "#8b949e";
  if (score > -0.5) return "#ff7b72";
  return "#f85149";
}

export default function SocialGraph({
  agents: propAgents,
  relationships: propRelationships,
  useMock = false,
}: SocialGraphProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [selectedEdge, setSelectedEdge] = useState<SocialLink | null>(null);

  const agents = useMock ? MOCK_AGENTS : propAgents;

  // Build mock relationships from the mock agent detail.
  const relationships = useMemo(() => {
    if (!useMock) return propRelationships;
    const map = new Map<string, Record<string, string | undefined>>();
    // Kora's relationships.
    map.set("01945c2a-3b4f-7def-8a12-bc34567890a1", MOCK_AGENT_DETAIL.state?.relationships ?? {});
    // Add reciprocal and extra ones.
    map.set("01945c2a-3b4f-7def-8a12-bc34567890a2", {
      "01945c2a-3b4f-7def-8a12-bc34567890a1": "0.70",
      "01945c2a-3b4f-7def-8a12-bc34567890a5": "0.50",
      "01945c2a-3b4f-7def-8a12-bc34567890a7": "0.60",
    });
    map.set("01945c2a-3b4f-7def-8a12-bc34567890a3", {
      "01945c2a-3b4f-7def-8a12-bc34567890a1": "0.30",
      "01945c2a-3b4f-7def-8a12-bc34567890a4": "0.40",
    });
    map.set("01945c2a-3b4f-7def-8a12-bc34567890a4", {
      "01945c2a-3b4f-7def-8a12-bc34567890a3": "0.40",
      "01945c2a-3b4f-7def-8a12-bc34567890a9": "0.80",
    });
    map.set("01945c2a-3b4f-7def-8a12-bc34567890a5", {
      "01945c2a-3b4f-7def-8a12-bc34567890a1": "0.55",
      "01945c2a-3b4f-7def-8a12-bc34567890a2": "0.50",
    });
    map.set("01945c2a-3b4f-7def-8a12-bc34567890a6", {
      "01945c2a-3b4f-7def-8a12-bc34567890a3": "-0.30",
    });
    return map;
  }, [useMock, propRelationships]);

  const { nodes, links } = useMemo(() => {
    const aliveAgents = agents.filter((a) => a.alive);
    const connectionCounts = new Map<string, number>();
    const linkSet = new Set<string>();
    const socialLinks: SocialLink[] = [];

    for (const [agentId, rels] of relationships) {
      for (const [targetId, scoreStr] of Object.entries(rels)) {
        if (!scoreStr) continue;
        // Deduplicate edges (use sorted pair as key).
        const key = [agentId, targetId].sort().join(":");
        if (linkSet.has(key)) continue;
        linkSet.add(key);

        const score = parseFloat(scoreStr);
        socialLinks.push({
          source: agentId,
          target: targetId,
          score,
          sourceId: agentId,
          targetId: targetId,
        });

        connectionCounts.set(agentId, (connectionCounts.get(agentId) ?? 0) + 1);
        connectionCounts.set(targetId, (connectionCounts.get(targetId) ?? 0) + 1);
      }
    }

    const agentIdSet = new Set(aliveAgents.map((a) => a.id));
    const socialNodes: SocialNode[] = aliveAgents.map((a) => ({
      id: a.id,
      name: a.name,
      alive: a.alive,
      connectionCount: connectionCounts.get(a.id) ?? 0,
    }));

    // Only keep links between existing agents.
    const filteredLinks = socialLinks.filter(
      (l) => agentIdSet.has(l.sourceId) && agentIdSet.has(l.targetId),
    );

    return { nodes: socialNodes, links: filteredLinks };
  }, [agents, relationships]);

  const agentNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agents) {
      map.set(a.id, a.name);
    }
    return map;
  }, [agents]);

  const renderGraph = useCallback(() => {
    const svgEl = svgRef.current;
    const container = containerRef.current;
    if (!svgEl || !container) return;

    const svg = d3.select(svgEl);
    const width = container.clientWidth;
    const height = container.clientHeight;

    svg.attr("viewBox", `0 0 ${width} ${height}`);
    svg.selectAll("*").remove();

    if (nodes.length === 0) {
      svg
        .append("text")
        .attr("x", width / 2)
        .attr("y", height / 2)
        .attr("text-anchor", "middle")
        .attr("fill", "#484f58")
        .attr("font-family", "var(--font-mono)")
        .attr("font-size", "14px")
        .text("No social relationships yet");
      return;
    }

    const g = svg.append("g");

    const zoom = d3
      .zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.3, 4])
      .on("zoom", (event: d3.D3ZoomEvent<SVGSVGElement, unknown>) => {
        g.attr("transform", event.transform.toString());
      });

    svg.call(zoom);

    const simulation = d3
      .forceSimulation<SocialNode>(nodes)
      .force(
        "link",
        d3
          .forceLink<SocialNode, SocialLink>(links)
          .id((d) => d.id)
          .distance(80),
      )
      .force("charge", d3.forceManyBody().strength(-200))
      .force("center", d3.forceCenter(width / 2, height / 2))
      .force("collision", d3.forceCollide(25));

    // Links.
    const link = g
      .append("g")
      .selectAll<SVGLineElement, SocialLink>("line")
      .data(links)
      .join("line")
      .attr("stroke", (d) => scoreToColor(d.score))
      .attr("stroke-width", (d) => Math.max(1, Math.abs(d.score) * 3))
      .attr("opacity", 0.6)
      .attr("cursor", "pointer")
      .on("click", (_event, d) => {
        setSelectedEdge(d);
      });

    // Nodes.
    const node = g
      .append("g")
      .selectAll<SVGGElement, SocialNode>("g")
      .data(nodes)
      .join("g")
      .attr("cursor", "grab")
      .call(
        d3
          .drag<SVGGElement, SocialNode>()
          .on("start", (event, d) => {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            d.fx = d.x;
            d.fy = d.y;
          })
          .on("drag", (event, d) => {
            d.fx = event.x;
            d.fy = event.y;
          })
          .on("end", (event, d) => {
            if (!event.active) simulation.alphaTarget(0);
            d.fx = null;
            d.fy = null;
          }),
      );

    node
      .append("circle")
      .attr("r", (d) => 8 + d.connectionCount * 2)
      .attr("fill", "var(--color-text-accent)")
      .attr("fill-opacity", 0.3)
      .attr("stroke", "var(--color-text-accent)")
      .attr("stroke-width", 1.5);

    node
      .append("text")
      .attr("dy", -14)
      .attr("text-anchor", "middle")
      .attr("fill", "var(--color-text-primary)")
      .attr("font-size", "10px")
      .attr("font-family", "var(--font-mono)")
      .text((d) => d.name);

    simulation.on("tick", () => {
      link
        .attr("x1", (d) => (d.source as SocialNode).x ?? 0)
        .attr("y1", (d) => (d.source as SocialNode).y ?? 0)
        .attr("x2", (d) => (d.target as SocialNode).x ?? 0)
        .attr("y2", (d) => (d.target as SocialNode).y ?? 0);

      node.attr("transform", (d) => `translate(${d.x ?? 0},${d.y ?? 0})`);
    });

    return () => {
      simulation.stop();
    };
  }, [nodes, links]);

  useEffect(() => {
    const cleanup = renderGraph();
    return () => cleanup?.();
  }, [renderGraph]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(() => {
      renderGraph();
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [renderGraph]);

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>Social Network</span>
        <span className="text-xs font-normal">
          {nodes.length} agents / {links.length} connections
        </span>
      </div>
      <div ref={containerRef} className="flex-1 relative p-0">
        <svg ref={svgRef} className="w-full h-full block" />

        {/* Selected edge detail */}
        {selectedEdge && (
          <div className="absolute top-2 right-2 bg-bg-elevated border border-border-primary rounded-sm px-md py-sm font-mono text-xs">
            <div className="mb-1">
              {agentNameMap.get(selectedEdge.sourceId) ?? "?"} --{" "}
              {agentNameMap.get(selectedEdge.targetId) ?? "?"}
            </div>
            <div>
              Score:{" "}
              <span className="font-semibold" style={{ color: scoreToColor(selectedEdge.score) }}>
                {formatDecimal(String(selectedEdge.score), 2)}
              </span>
            </div>
            <button
              onClick={() => setSelectedEdge(null)}
              className="mt-1 bg-transparent border-none text-text-muted cursor-pointer text-xs p-0"
            >
              close
            </button>
          </div>
        )}

        {/* Legend */}
        <div className="absolute bottom-2 left-2 flex gap-3 text-xs font-mono text-text-secondary">
          <span className="flex items-center gap-1">
            <span className="inline-block w-4 h-0.5 bg-positive" />
            Positive
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block w-4 h-0.5 bg-neutral" />
            Neutral
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block w-4 h-0.5 bg-negative" />
            Negative
          </span>
        </div>
      </div>
    </div>
  );
}

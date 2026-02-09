/**
 * World Map Visualization (Task 4.5.9)
 *
 * Clean schematic continent map rendered as layered SVG with fixed-coordinate
 * location nodes connected by curved route paths. Strategy-minimap style --
 * data-driven, no terrain decoration.
 *
 * Layers (back to front):
 *   1. Ocean background
 *   2. Continental shelf glow
 *   3. Continent landmass with region tints
 *   4. River
 *   5. Region border lines and labels
 *   6. Route paths (quadratic Bezier curves)
 *   7. Location nodes with agent indicators
 */
import { useCallback, useEffect, useMemo, useRef } from "react";

import * as d3 from "d3";

import type { AgentListItem, LocationListItem } from "../types/generated/index.ts";
import { MOCK_AGENTS, MOCK_LOCATIONS, MOCK_ROUTES, type MockRoute } from "../utils/mockData.ts";

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

interface WorldMapProps {
  locations: LocationListItem[];
  agents: AgentListItem[];
  routes: MockRoute[];
  onSelectLocation: (id: string) => void;
  useMock?: boolean;
}

// ---------------------------------------------------------------------------
// Region color palette
// ---------------------------------------------------------------------------

const REGION_COLORS: Record<string, string> = {
  "Central Valley": "#58a6ff",
  Highlands: "#bc8cff",
  "Coastal Lowlands": "#3fb950",
};

// ---------------------------------------------------------------------------
// Route style helpers
// ---------------------------------------------------------------------------

function getPathWidth(pathType: string): number {
  switch (pathType) {
    case "Highway":
      return 4;
    case "Road":
      return 3;
    case "WornPath":
      return 2.5;
    case "DirtTrail":
      return 1.5;
    default:
      return 0.8;
  }
}

function getPathDash(pathType: string): string {
  switch (pathType) {
    case "Highway":
    case "Road":
      return "";
    case "WornPath":
      return "4,2";
    case "DirtTrail":
      return "3,3";
    default:
      return "2,4";
  }
}

// ---------------------------------------------------------------------------
// Coordinate system -- landscape 1400 x 600 viewBox
// ---------------------------------------------------------------------------

interface Coord {
  x: number;
  y: number;
}

const LOCATION_COORDS: Record<string, Coord> = {
  // Central Valley
  "01945c2a-3b4f-7def-8a12-bc34567890c1": { x: 760, y: 300 }, // Riverbank
  "01945c2a-3b4f-7def-8a12-bc34567890c2": { x: 560, y: 257 }, // Forest Edge
  "01945c2a-3b4f-7def-8a12-bc34567890c3": { x: 920, y: 329 }, // Open Field
  // Highlands
  "01945c2a-3b4f-7def-8a12-bc34567890c4": { x: 680, y: 170 }, // Rocky Outcrop
  "01945c2a-3b4f-7def-8a12-bc34567890c5": { x: 840, y: 134 }, // Mountain Cave
  "01945c2a-3b4f-7def-8a12-bc34567890c6": { x: 520, y: 156 }, // Hilltop
  // Coastal Lowlands
  "01945c2a-3b4f-7def-8a12-bc34567890c7": { x: 400, y: 422 }, // Beach
  "01945c2a-3b4f-7def-8a12-bc34567890c8": { x: 240, y: 458 }, // Tidal Pools
  "01945c2a-3b4f-7def-8a12-bc34567890c9": { x: 560, y: 444 }, // Estuary
};

const REGION_CENTERS: Record<string, Coord> = {
  "Central Valley": { x: 740, y: 296 },
  Highlands: { x: 680, y: 152 },
  "Coastal Lowlands": { x: 400, y: 440 },
};

function coordForLocation(id: string, region: string, index: number): Coord {
  // eslint-disable-next-line security/detect-object-injection -- id is a UUID from typed LocationListItem
  const known = LOCATION_COORDS[id];
  if (known) return known;
  // eslint-disable-next-line security/detect-object-injection -- region is from typed LocationListItem
  const center = REGION_CENTERS[region] ?? { x: 700, y: 300 };
  const angle = ((index * 137.5) % 360) * (Math.PI / 180);
  const radius = 40 + (index % 3) * 20;
  return {
    x: center.x + Math.cos(angle) * radius,
    y: center.y + Math.sin(angle) * radius,
  };
}

// ---------------------------------------------------------------------------
// SVG path data -- landscape continent outline and features
// ---------------------------------------------------------------------------

/** Fictional continent -- landscape oriented, irregular coastline. */
const CONTINENT_PATH = [
  "M110,140",
  // North coast -- rugged highlands shore
  "C136,127 190,116 250,109",
  "C316,98 370,104 410,95",
  "C464,85 510,78 560,73",
  // Northern peninsula
  "C604,66 636,61 676,64",
  "C704,57 724,54 750,59",
  "C784,54 816,61 856,66",
  // NE coast
  "C896,61 936,66 980,75",
  "C1030,85 1084,98 1130,111",
  "C1176,126 1216,140 1250,157",
  // East coast
  "C1284,176 1304,198 1310,221",
  "C1316,244 1304,265 1300,287",
  "C1296,309 1304,327 1310,349",
  "C1316,371 1304,392 1284,412",
  // SE corner
  "C1256,431 1224,445 1190,457",
  // South coast
  "C1144,469 1096,479 1044,484",
  "C984,491 924,496 864,498",
  "C804,502 750,503 696,500",
  "C636,503 584,505 544,502",
  "C496,498 444,493 404,486",
  // SW bulge -- accommodates coastal lowlands
  "C356,479 324,474 296,476",
  "C256,481 216,487 184,493",
  "C144,496 116,493 96,486",
  "C64,476 50,464 56,450",
  // West coast
  "C64,431 70,412 64,392",
  "C56,371 50,349 60,327",
  "C56,306 50,284 60,263",
  "C70,241 76,219 84,199",
  "C96,176 104,160 110,140",
  "Z",
].join(" ");

/** Wavy border between Highlands and Central Valley. */
const HIGHLANDS_BORDER =
  "M84,219 C240,227 420,212 600,222 C760,213 920,224 1080,217 C1200,221 1280,219 1310,219";

/** Wavy border between Central Valley and Coastal Lowlands. */
const VALLEY_BORDER =
  "M60,385 C220,392 400,378 600,388 C760,379 920,390 1080,383 C1200,386 1280,385 1310,385";

/** River from highlands through valley to estuary. */
const RIVER_PATH =
  "M740,150 C736,178 730,206 736,235 " +
  "C744,264 756,286 760,300 " +
  "C764,322 750,343 730,365 " +
  "C704,390 650,415 590,435 " +
  "C576,440 564,443 560,444";

const REGION_LABELS: { name: string; x: number; y: number; color: string }[] = [
  { name: "HIGHLANDS", x: 720, y: 113, color: "rgba(188, 140, 255, 0.2)" },
  { name: "CENTRAL VALLEY", x: 740, y: 278, color: "rgba(88, 166, 255, 0.2)" },
  { name: "COASTAL LOWLANDS", x: 560, y: 476, color: "rgba(63, 185, 80, 0.2)" },
];

// ---------------------------------------------------------------------------
// Route Bezier helpers
// ---------------------------------------------------------------------------

function routeBezier(from: Coord, to: Coord): string {
  const mx = (from.x + to.x) / 2;
  const my = (from.y + to.y) / 2;
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const dist = Math.sqrt(dx * dx + dy * dy);
  const offset = Math.min(dist * 0.15, 30);
  const nx = -dy / (dist || 1);
  const ny = dx / (dist || 1);
  const cx = mx + nx * offset;
  const cy = my + ny * offset;
  return `M${from.x},${from.y} Q${cx},${cy} ${to.x},${to.y}`;
}

function routeMidpoint(from: Coord, to: Coord): Coord {
  const mx = (from.x + to.x) / 2;
  const my = (from.y + to.y) / 2;
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const dist = Math.sqrt(dx * dx + dy * dy);
  const offset = Math.min(dist * 0.15, 30) * 0.5;
  const nx = -dy / (dist || 1);
  const ny = dx / (dist || 1);
  return { x: mx + nx * offset, y: my + ny * offset };
}

// ---------------------------------------------------------------------------
// Location node renderer (extracted to reduce cognitive complexity)
// ---------------------------------------------------------------------------

interface PositionedLocation extends LocationListItem {
  coord: Coord;
  agentCount: number;
}

function drawLocationNode(
  layer: d3.Selection<SVGGElement, unknown, null, undefined>,
  loc: PositionedLocation,
  onSelect: (id: string) => void,
): void {
  const nodeG = layer
    .append("g")
    .attr("cursor", "pointer")
    .on("click", () => {
      onSelect(loc.id);
    });

  const regionColor = REGION_COLORS[loc.region] ?? "#8b949e";
  const hasAgents = loc.agentCount > 0;
  const circleR = 12 + loc.agentCount * 2;

  // Glow for locations with agents
  if (hasAgents) {
    nodeG
      .append("circle")
      .attr("cx", loc.coord.x)
      .attr("cy", loc.coord.y)
      .attr("r", circleR + 6)
      .attr("fill", regionColor)
      .attr("fill-opacity", 0.1)
      .attr("filter", "url(#glow)");
  }

  // Location circle
  nodeG
    .append("circle")
    .attr("cx", loc.coord.x)
    .attr("cy", loc.coord.y)
    .attr("r", circleR)
    .attr("fill", regionColor)
    .attr("fill-opacity", 0.2)
    .attr("stroke", regionColor)
    .attr("stroke-width", 1.5);

  // Name label
  nodeG
    .append("text")
    .attr("x", loc.coord.x)
    .attr("y", loc.coord.y - 20)
    .attr("text-anchor", "middle")
    .attr("fill", "#c9d1d9")
    .attr("font-size", "11px")
    .attr("font-family", "var(--font-mono)")
    .attr("font-weight", "600")
    .text(loc.name);

  // Agent count badge + orbital dots
  if (hasAgents) {
    nodeG
      .append("circle")
      .attr("cx", loc.coord.x + 14)
      .attr("cy", loc.coord.y - 10)
      .attr("r", 7)
      .attr("fill", "#f0c040")
      .attr("stroke", "#0d1117")
      .attr("stroke-width", 1.5);

    nodeG
      .append("text")
      .attr("x", loc.coord.x + 14)
      .attr("y", loc.coord.y - 7)
      .attr("text-anchor", "middle")
      .attr("fill", "#0d1117")
      .attr("font-size", "9px")
      .attr("font-weight", "700")
      .attr("font-family", "var(--font-mono)")
      .text(loc.agentCount);

    const dotCount = Math.min(loc.agentCount, 6);
    for (let i = 0; i < dotCount; i++) {
      const angle = (i / dotCount) * Math.PI * 2;
      const orbitR = 8 + loc.agentCount;
      nodeG
        .append("circle")
        .attr("cx", loc.coord.x + Math.cos(angle) * orbitR)
        .attr("cy", loc.coord.y + Math.sin(angle) * orbitR)
        .attr("r", 3)
        .attr("fill", "#f0c040")
        .attr("opacity", 0.8);
    }
  }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function WorldMap({
  locations: propLocations,
  agents: propAgents,
  routes: propRoutes,
  onSelectLocation,
  useMock = false,
}: WorldMapProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const locations = useMock ? MOCK_LOCATIONS : propLocations;
  const agents = useMock ? MOCK_AGENTS : propAgents;
  const routes = useMock ? MOCK_ROUTES : propRoutes;

  const agentCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const agent of agents) {
      if (agent.alive && agent.location_id) {
        counts[agent.location_id] = (counts[agent.location_id] ?? 0) + 1;
      }
    }
    return counts;
  }, [agents]);

  const positionedLocations = useMemo(() => {
    return locations.map((loc, idx) => ({
      ...loc,
      coord: coordForLocation(loc.id, loc.region, idx),
      agentCount: agentCounts[loc.id] ?? 0,
    }));
  }, [locations, agentCounts]);

  const coordLookup = useMemo(() => {
    const map = new Map<string, Coord>();
    for (const loc of positionedLocations) {
      map.set(loc.id, loc.coord);
    }
    return map;
  }, [positionedLocations]);

  const validRoutes = useMemo(() => {
    return routes.filter((r) => coordLookup.has(r.from) && coordLookup.has(r.to));
  }, [routes, coordLookup]);

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  const renderMap = useCallback(() => {
    const svgEl = svgRef.current;
    const container = containerRef.current;
    if (!svgEl || !container) return;

    const svg = d3.select(svgEl);
    svg.attr("viewBox", "0 0 1400 600").attr("preserveAspectRatio", "xMidYMid meet");
    svg.selectAll("*").remove();

    if (positionedLocations.length === 0) {
      svg
        .append("text")
        .attr("x", 700)
        .attr("y", 300)
        .attr("text-anchor", "middle")
        .attr("fill", "#484f58")
        .attr("font-family", "var(--font-mono)")
        .attr("font-size", "14px")
        .text("No locations to display");
      return;
    }

    // --- SVG Definitions ---
    const defs = svg.append("defs");

    // Continental shelf blur
    defs
      .append("filter")
      .attr("id", "shelf-blur")
      .attr("filterUnits", "userSpaceOnUse")
      .attr("x", "-50%")
      .attr("y", "-50%")
      .attr("width", "200%")
      .attr("height", "200%")
      .append("feGaussianBlur")
      .attr("stdDeviation", "20");

    // Glow filter for active locations
    const glowFilter = defs
      .append("filter")
      .attr("id", "glow")
      .attr("filterUnits", "userSpaceOnUse")
      .attr("x", "-100%")
      .attr("y", "-100%")
      .attr("width", "300%")
      .attr("height", "300%");
    glowFilter.append("feGaussianBlur").attr("stdDeviation", "4").attr("result", "blur");
    glowFilter
      .append("feMerge")
      .selectAll("feMergeNode")
      .data(["blur", "SourceGraphic"])
      .join("feMergeNode")
      .attr("in", (d) => d);

    // River glow filter
    defs
      .append("filter")
      .attr("id", "river-glow")
      .attr("filterUnits", "userSpaceOnUse")
      .attr("x", "-50%")
      .attr("y", "-50%")
      .attr("width", "200%")
      .attr("height", "200%")
      .append("feGaussianBlur")
      .attr("stdDeviation", "3");

    // Continent clip path
    defs.append("clipPath").attr("id", "continent-clip").append("path").attr("d", CONTINENT_PATH);

    // --- Root group ---
    const g = svg.append("g");

    // Layer 1: Ocean background
    g.append("rect").attr("width", 1400).attr("height", 600).attr("fill", "#060e1a");

    // Layer 2: Continental shelf glow
    g.append("path")
      .attr("d", CONTINENT_PATH)
      .attr("fill", "#0d1f35")
      .attr("filter", "url(#shelf-blur)");

    // Layer 3: Continent landmass
    g.append("path")
      .attr("d", CONTINENT_PATH)
      .attr("fill", "#121c2b")
      .attr("stroke", "#1e3a55")
      .attr("stroke-width", 2);

    // Region tints (clipped to continent)
    const regionTints = g.append("g").attr("clip-path", "url(#continent-clip)");
    regionTints
      .append("rect")
      .attr("x", 0)
      .attr("y", 0)
      .attr("width", 1400)
      .attr("height", 224)
      .attr("fill", "rgba(188, 140, 255, 0.12)");
    regionTints
      .append("rect")
      .attr("x", 0)
      .attr("y", 224)
      .attr("width", 1400)
      .attr("height", 162)
      .attr("fill", "rgba(88, 166, 255, 0.12)");
    regionTints
      .append("rect")
      .attr("x", 0)
      .attr("y", 386)
      .attr("width", 1400)
      .attr("height", 214)
      .attr("fill", "rgba(63, 185, 80, 0.12)");

    // Layer 4: River (clipped to continent)
    const terrain = g.append("g").attr("clip-path", "url(#continent-clip)");

    terrain
      .append("path")
      .attr("d", RIVER_PATH)
      .attr("fill", "none")
      .attr("stroke", "rgba(88, 166, 255, 0.15)")
      .attr("stroke-width", 6)
      .attr("filter", "url(#river-glow)");

    terrain
      .append("path")
      .attr("d", RIVER_PATH)
      .attr("fill", "none")
      .attr("stroke", "rgba(88, 166, 255, 0.35)")
      .attr("stroke-width", 2.5)
      .attr("stroke-linecap", "round");

    // Layer 5: Region borders and labels
    const borders = g.append("g").attr("clip-path", "url(#continent-clip)");

    borders
      .append("path")
      .attr("d", HIGHLANDS_BORDER)
      .attr("fill", "none")
      .attr("stroke", "rgba(255, 255, 255, 0.06)")
      .attr("stroke-width", 1)
      .attr("stroke-dasharray", "8,5");

    borders
      .append("path")
      .attr("d", VALLEY_BORDER)
      .attr("fill", "none")
      .attr("stroke", "rgba(255, 255, 255, 0.06)")
      .attr("stroke-width", 1)
      .attr("stroke-dasharray", "8,5");

    for (const label of REGION_LABELS) {
      borders
        .append("text")
        .attr("x", label.x)
        .attr("y", label.y)
        .attr("text-anchor", "middle")
        .attr("fill", label.color)
        .attr("font-size", "10px")
        .attr("font-family", "var(--font-mono)")
        .attr("font-weight", "600")
        .attr("letter-spacing", "4px")
        .text(label.name);
    }

    // Layer 6: Routes (quadratic Bezier curves)
    const routeLayer = g.append("g");

    for (const route of validRoutes) {
      const fromCoord = coordLookup.get(route.from);
      const toCoord = coordLookup.get(route.to);
      if (!fromCoord || !toCoord) continue;

      routeLayer
        .append("path")
        .attr("d", routeBezier(fromCoord, toCoord))
        .attr("fill", "none")
        .attr("stroke", "#3a4858")
        .attr("stroke-width", getPathWidth(route.pathType))
        .attr("stroke-dasharray", getPathDash(route.pathType))
        .attr("opacity", 0.8);

      const mid = routeMidpoint(fromCoord, toCoord);
      routeLayer
        .append("text")
        .attr("x", mid.x)
        .attr("y", mid.y)
        .attr("text-anchor", "middle")
        .attr("fill", "#4a5568")
        .attr("font-size", "9px")
        .attr("font-family", "var(--font-mono)")
        .text(`${route.cost}t`);
    }

    // Layer 7: Location nodes
    const nodeLayer = g.append("g");

    for (const loc of positionedLocations) {
      drawLocationNode(nodeLayer, loc, onSelectLocation);
    }
  }, [positionedLocations, validRoutes, coordLookup, onSelectLocation]);

  // -------------------------------------------------------------------------
  // Effects
  // -------------------------------------------------------------------------

  useEffect(() => {
    renderMap();
  }, [renderMap]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(() => {
      renderMap();
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [renderMap]);

  // -------------------------------------------------------------------------
  // JSX
  // -------------------------------------------------------------------------

  return (
    <div className="flex-1 min-h-0 flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>World Map</span>
        <span className="text-xs font-normal">
          {locations.length} locations / {agents.filter((a) => a.alive).length} agents
        </span>
      </div>
      <div ref={containerRef} className="flex-1 min-h-0 relative overflow-hidden p-0 bg-ocean">
        <svg ref={svgRef} className="block w-full h-full" />
        {/* Legend */}
        <div className="absolute bottom-2 left-2 flex gap-3 text-xs font-mono text-text-secondary">
          {Object.entries(REGION_COLORS).map(([region, color]) => (
            <span key={region} className="flex items-center gap-1">
              <span className="w-2 h-2 rounded-full inline-block" style={{ background: color }} />
              {region}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

/**
 * World Map Visualization (Task 4.5.9 + Phase 9.5 Enhancements)
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
 *   6. Agent movement trails (Phase 9.5.3)
 *   7. Route paths (quadratic Bezier curves, color-coded by type)
 *   8. Location nodes with agent indicators
 *   9. Agent name labels (Phase 9.5.5, togglable)
 *
 * Overlays (HTML positioned over SVG):
 *   - Location detail popup (Phase 9.5.2)
 *   - Resource heatmap legend (Phase 9.5.4)
 *   - Toggle controls toolbar
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import * as d3 from "d3";

import type {
  AgentListItem,
  Event,
  LocationDetailResponse,
  LocationListItem,
  Route,
} from "../types/generated/index.ts";
import { cn } from "../lib/utils.ts";
import type { ChartTooltipData } from "./ui/chart-tooltip.tsx";
import { ChartTooltip } from "./ui/chart-tooltip.tsx";

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/** Mapped route for display -- uses API Route type directly now. */
interface MappedRoute {
  from: string;
  to: string;
  cost: number;
  pathType: string;
  durability: number;
  maxDurability: number;
}

interface WorldMapProps {
  locations: LocationListItem[];
  agents: AgentListItem[];
  routes: Route[];
  events: Event[];
  currentTick: number;
  selectedLocationId: string | null;
  locationDetail: LocationDetailResponse | null;
  agentNames: Map<string, string>;
  onSelectLocation: (id: string) => void;
  onSelectAgent: (id: string) => void;
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

function getPathColor(pathType: string): string {
  switch (pathType) {
    case "Highway":
      return "#8b949e";
    case "Road":
      return "#6e7681";
    case "WornPath":
      return "#5a4a3a";
    case "DirtTrail":
      return "#6b5b45";
    default:
      return "#3a4858";
  }
}

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
// Agent color assignment (stable per-agent hash)
// ---------------------------------------------------------------------------

const AGENT_PALETTE = [
  "#58a6ff", "#3fb950", "#f0c040", "#bc8cff", "#ff7b72",
  "#79c0ff", "#db6d28", "#f778ba", "#a5d6a7", "#d29922",
  "#b0bec5", "#c9733a",
];

function agentColor(agentId: string): string {
  let hash = 0;
  for (let i = 0; i < agentId.length; i++) {
    hash = ((hash << 5) - hash + agentId.charCodeAt(i)) | 0;
  }
  return AGENT_PALETTE[Math.abs(hash) % AGENT_PALETTE.length] ?? "#8b949e";
}

// ---------------------------------------------------------------------------
// Heatmap resource color mapping
// ---------------------------------------------------------------------------

interface ResourceHeatmapData {
  dominantColor: string;
  intensity: number;
  label: string;
}

function computeHeatmapForLocation(
  detail: LocationDetailResponse | null,
  locationId: string,
  _locations: LocationListItem[],
): ResourceHeatmapData | null {
  // If we have the full location detail, compute from it
  if (detail && detail.location.id === locationId) {
    const resources = detail.location.base_resources;
    let maxAvail = 0;
    let dominantResource = "";
    let totalMax = 0;
    let totalAvail = 0;

    for (const [resName, node] of Object.entries(resources)) {
      if (!node) continue;
      totalAvail += node.available;
      totalMax += node.max_capacity;
      if (node.available > maxAvail) {
        maxAvail = node.available;
        dominantResource = resName;
      }
    }

    if (totalMax === 0) return { dominantColor: "#484f58", intensity: 0, label: "Empty" };

    const intensity = totalMax > 0 ? totalAvail / totalMax : 0;
    const color = getResourceHeatColor(dominantResource);
    return { dominantColor: color, intensity, label: dominantResource };
  }
  return null;
}

function getResourceHeatColor(resource: string): string {
  if (resource.startsWith("Food")) return "#3fb950"; // green
  if (resource === "Water") return "#58a6ff"; // blue
  if (resource === "Wood" || resource === "Stone" || resource === "Ore" || resource === "Metal")
    return "#8b6914"; // brown
  if (resource === "Fiber" || resource === "Clay" || resource === "Hide")
    return "#8b6914"; // brown
  return "#484f58"; // gray
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
// Movement trail extraction from events
// ---------------------------------------------------------------------------

interface MovementTrail {
  agentId: string;
  fromLocationId: string;
  toLocationId: string;
  tick: number;
}

function extractMovementTrails(
  events: Event[],
  currentTick: number,
  trailWindow: number,
): MovementTrail[] {
  const trails: MovementTrail[] = [];
  const minTick = currentTick - trailWindow;

  for (const event of events) {
    if (event.tick < minTick) continue;
    if (event.event_type !== "ActionSucceeded") continue;
    if (!event.agent_id || !event.details) continue;

    const details = event.details as Record<string, unknown>;
    if (typeof details.action_type !== "string") continue;
    // The action_type in observer events is formatted as e.g. "Move"
    if (!details.action_type.includes("Move")) continue;

    // For Move actions, the agent moved from the event's location_id
    // to their current location. We track the from-location from the
    // agent_state_snapshot.
    if (event.agent_state_snapshot && event.location_id) {
      trails.push({
        agentId: event.agent_id,
        fromLocationId: event.location_id,
        toLocationId: event.agent_state_snapshot.location_id,
        tick: event.tick,
      });
    }
  }

  return trails;
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
  isSelected: boolean,
  heatmapMode: boolean,
  onSelect: (id: string) => void,
  onHover?: (event: MouseEvent, loc: PositionedLocation) => void,
  onLeave?: () => void,
): void {
  const nodeG = layer
    .append("g")
    .attr("cursor", "pointer")
    .on("click", () => {
      onSelect(loc.id);
    })
    .on("mouseenter", (event: MouseEvent) => onHover?.(event, loc))
    .on("mouseleave", () => onLeave?.());

  const regionColor = REGION_COLORS[loc.region] ?? "#8b949e";
  const hasAgents = loc.agentCount > 0;
  const circleR = 12 + loc.agentCount * 2;

  // Selection ring
  if (isSelected) {
    nodeG
      .append("circle")
      .attr("cx", loc.coord.x)
      .attr("cy", loc.coord.y)
      .attr("r", circleR + 10)
      .attr("fill", "none")
      .attr("stroke", "#f0c040")
      .attr("stroke-width", 2)
      .attr("stroke-dasharray", "4,3")
      .attr("opacity", 0.9);
  }

  // Glow for locations with agents
  if (hasAgents && !heatmapMode) {
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
  const fillColor = heatmapMode ? regionColor : regionColor;
  nodeG
    .append("circle")
    .attr("cx", loc.coord.x)
    .attr("cy", loc.coord.y)
    .attr("r", circleR)
    .attr("fill", fillColor)
    .attr("fill-opacity", heatmapMode ? 0.4 : 0.2)
    .attr("stroke", regionColor)
    .attr("stroke-width", isSelected ? 2.5 : 1.5);

  // Name label
  nodeG
    .append("text")
    .attr("x", loc.coord.x)
    .attr("y", loc.coord.y - circleR - 6)
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
      .attr("cx", loc.coord.x + circleR)
      .attr("cy", loc.coord.y - circleR + 4)
      .attr("r", 7)
      .attr("fill", "#f0c040")
      .attr("stroke", "#0d1117")
      .attr("stroke-width", 1.5);

    nodeG
      .append("text")
      .attr("x", loc.coord.x + circleR)
      .attr("y", loc.coord.y - circleR + 7)
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
// Agent label renderer (Phase 9.5.5)
// ---------------------------------------------------------------------------

function drawAgentLabels(
  layer: d3.Selection<SVGGElement, unknown, null, undefined>,
  agents: AgentListItem[],
  coordLookup: Map<string, Coord>,
  agentCounts: Record<string, number>,
): void {
  // Group alive agents by location
  const agentsByLocation = new Map<string, AgentListItem[]>();
  for (const agent of agents) {
    if (!agent.alive || !agent.location_id) continue;
    const list = agentsByLocation.get(agent.location_id) ?? [];
    list.push(agent);
    agentsByLocation.set(agent.location_id, list);
  }

  for (const [locationId, locAgents] of agentsByLocation) {
    const coord = coordLookup.get(locationId);
    if (!coord) continue;

    const count = agentCounts[locationId] ?? 0;
    const circleR = 12 + count * 2;

    for (let i = 0; i < locAgents.length; i++) {
      const agent = locAgents[i];
      if (!agent) continue;
      const color = agentColor(agent.id);
      const yOffset = circleR + 14 + i * 12;

      layer
        .append("text")
        .attr("x", coord.x)
        .attr("y", coord.y + yOffset)
        .attr("text-anchor", "middle")
        .attr("fill", color)
        .attr("font-size", "9px")
        .attr("font-family", "var(--font-mono)")
        .attr("font-weight", "400")
        .attr("opacity", 0.85)
        .text(agent.name);
    }
  }
}

// ---------------------------------------------------------------------------
// Location Detail Popup Component (Phase 9.5.2)
// ---------------------------------------------------------------------------

function LocationDetailPopup({
  locationDetail,
  coord,
  containerRect,
  onClose,
  onSelectAgent,
}: {
  locationDetail: LocationDetailResponse;
  coord: Coord;
  containerRect: DOMRect;
  onClose: () => void;
  onSelectAgent: (id: string) => void;
}) {
  // Convert SVG coords to screen position
  // The SVG viewBox is 1400x600, mapped to the container rect
  const scaleX = containerRect.width / 1400;
  const scaleY = containerRect.height / 600;
  const screenX = coord.x * scaleX;
  const screenY = coord.y * scaleY;

  // Position popup to the right of the node, or left if too close to right edge
  const popupWidth = 280;
  const flipX = screenX + popupWidth + 30 > containerRect.width;
  const left = flipX ? screenX - popupWidth - 20 : screenX + 30;
  const top = Math.max(8, Math.min(screenY - 40, containerRect.height - 300));

  const loc = locationDetail.location;
  const agentsHere = locationDetail.agents_here;
  const resources = Object.entries(loc.base_resources).filter(
    (entry): entry is [string, NonNullable<typeof entry[1]>] => entry[1] != null,
  );

  return (
    <div
      className="absolute z-50 bg-bg-elevated border border-border-primary rounded-md shadow-lg font-mono text-xs overflow-hidden"
      style={{ left: `${left}px`, top: `${top}px`, width: `${popupWidth}px` }}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-sm py-xs bg-bg-tertiary border-b border-border-primary">
        <span className="text-text-primary font-semibold text-sm truncate">{loc.name}</span>
        <button
          className="text-text-muted hover:text-text-primary text-base leading-none bg-transparent border-0 cursor-pointer px-xs"
          onClick={onClose}
        >
          x
        </button>
      </div>

      <div className="p-sm space-y-2 max-h-64 overflow-y-auto">
        {/* Region and description */}
        <div>
          <span className="text-text-secondary">{loc.region}</span>
          <span className="text-text-muted mx-1">/</span>
          <span className="text-text-secondary">{loc.location_type}</span>
        </div>
        <p className="text-text-muted text-2xs leading-tight">{loc.description}</p>

        {/* Capacity bar */}
        <div>
          <div className="flex justify-between text-text-secondary mb-0.5">
            <span>Capacity</span>
            <span>
              {agentsHere.length} / {loc.capacity}
            </span>
          </div>
          <div className="w-full h-1.5 bg-bg-primary rounded-sm overflow-hidden">
            <div
              className="h-full rounded-sm bg-info"
              style={{ width: `${Math.min(100, (agentsHere.length / Math.max(1, loc.capacity)) * 100)}%` }}
            />
          </div>
        </div>

        {/* Resources */}
        {resources.length > 0 && (
          <div>
            <div className="text-text-secondary font-semibold mb-1">Resources</div>
            <div className="space-y-1">
              {resources.map(([resName, node]) => {
                const pct =
                  node.max_capacity > 0 ? (node.available / node.max_capacity) * 100 : 0;
                return (
                  <div key={resName}>
                    <div className="flex justify-between text-text-muted text-2xs">
                      <span>{resName.replace(/([A-Z])/g, " $1").trim()}</span>
                      <span>
                        {node.available}/{node.max_capacity}
                      </span>
                    </div>
                    <div className="w-full h-1 bg-bg-primary rounded-sm overflow-hidden">
                      <div
                        className={cn(
                          "h-full rounded-sm",
                          pct > 50 ? "bg-success" : pct > 20 ? "bg-warning" : "bg-danger",
                        )}
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* Occupants */}
        {agentsHere.length > 0 && (
          <div>
            <div className="text-text-secondary font-semibold mb-1">Occupants</div>
            <div className="flex flex-wrap gap-1">
              {agentsHere.map((a) => (
                <button
                  key={a.id}
                  className="px-1.5 py-0.5 rounded-sm bg-bg-primary text-text-accent border border-border-secondary hover:bg-bg-tertiary cursor-pointer text-2xs font-mono"
                  onClick={() => onSelectAgent(a.id)}
                >
                  {a.name}
                </button>
              ))}
            </div>
          </div>
        )}
        {agentsHere.length === 0 && (
          <div className="text-text-muted text-2xs italic">No occupants</div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Heatmap legend component
// ---------------------------------------------------------------------------

function HeatmapLegend() {
  return (
    <div className="absolute top-2 right-2 bg-bg-elevated/90 border border-border-primary rounded-md px-sm py-xs font-mono text-2xs">
      <div className="text-text-secondary font-semibold mb-1">Resource Heatmap</div>
      <div className="space-y-0.5">
        <div className="flex items-center gap-1.5">
          <span className="w-2.5 h-2.5 rounded-sm inline-block" style={{ background: "#3fb950" }} />
          <span className="text-text-muted">Food-rich</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="w-2.5 h-2.5 rounded-sm inline-block" style={{ background: "#58a6ff" }} />
          <span className="text-text-muted">Water-rich</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="w-2.5 h-2.5 rounded-sm inline-block" style={{ background: "#8b6914" }} />
          <span className="text-text-muted">Material-rich</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="w-2.5 h-2.5 rounded-sm inline-block" style={{ background: "#484f58" }} />
          <span className="text-text-muted">Depleted</span>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function WorldMap({
  locations,
  agents,
  routes,
  events,
  currentTick,
  selectedLocationId,
  locationDetail,
  agentNames: _agentNames,
  onSelectLocation,
  onSelectAgent,
}: WorldMapProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Toggle states for overlays
  const [showLabels, setShowLabels] = useState(false);
  const [showTrails, setShowTrails] = useState(true);
  const [showHeatmap, setShowHeatmap] = useState(false);
  const [hoveredRoute, setHoveredRoute] = useState<string | null>(null);
  const [mapTooltip, setMapTooltip] = useState<ChartTooltipData | null>(null);

  // Agent count per location
  const agentCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const agent of agents) {
      if (agent.alive && agent.location_id) {
        counts[agent.location_id] = (counts[agent.location_id] ?? 0) + 1;
      }
    }
    return counts;
  }, [agents]);

  // Positioned locations with coordinates
  const positionedLocations = useMemo(() => {
    return locations.map((loc, idx) => ({
      ...loc,
      coord: coordForLocation(loc.id, loc.region, idx),
      agentCount: agentCounts[loc.id] ?? 0,
    }));
  }, [locations, agentCounts]);

  // Coordinate lookup map
  const coordLookup = useMemo(() => {
    const map = new Map<string, Coord>();
    for (const loc of positionedLocations) {
      map.set(loc.id, loc.coord);
    }
    return map;
  }, [positionedLocations]);

  // Map API routes to display routes
  const mappedRoutes = useMemo((): MappedRoute[] => {
    return routes.map((r) => ({
      from: r.from_location,
      to: r.to_location,
      cost: r.cost_ticks,
      pathType: r.path_type,
      durability: r.durability,
      maxDurability: r.max_durability,
    }));
  }, [routes]);

  const validRoutes = useMemo(() => {
    return mappedRoutes.filter((r) => coordLookup.has(r.from) && coordLookup.has(r.to));
  }, [mappedRoutes, coordLookup]);

  // Movement trails from events (Phase 9.5.3)
  const movementTrails = useMemo(() => {
    if (!showTrails) return [];
    return extractMovementTrails(events, currentTick, 10);
  }, [events, currentTick, showTrails]);

  // Heatmap data per location
  const heatmapData = useMemo(() => {
    if (!showHeatmap) return new Map<string, ResourceHeatmapData>();
    const map = new Map<string, ResourceHeatmapData>();
    for (const loc of positionedLocations) {
      const data = computeHeatmapForLocation(locationDetail, loc.id, locations);
      if (data) {
        map.set(loc.id, data);
      }
    }
    return map;
  }, [showHeatmap, positionedLocations, locationDetail, locations]);

  // Popup coordinate for selected location
  const selectedLocationCoord = useMemo(() => {
    if (!selectedLocationId) return null;
    return coordLookup.get(selectedLocationId) ?? null;
  }, [selectedLocationId, coordLookup]);

  // Container rect for popup positioning
  const [containerRect, setContainerRect] = useState<DOMRect | null>(null);
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const updateRect = () => setContainerRect(container.getBoundingClientRect());
    updateRect();
    const observer = new ResizeObserver(updateRect);
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

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

    // Heatmap glow (larger, colored)
    const heatGlow = defs
      .append("filter")
      .attr("id", "heat-glow")
      .attr("filterUnits", "userSpaceOnUse")
      .attr("x", "-100%")
      .attr("y", "-100%")
      .attr("width", "300%")
      .attr("height", "300%");
    heatGlow.append("feGaussianBlur").attr("stdDeviation", "12").attr("result", "blur");
    heatGlow
      .append("feMerge")
      .selectAll("feMergeNode")
      .data(["blur", "SourceGraphic"])
      .join("feMergeNode")
      .attr("in", (d) => d);

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

    // Layer 6: Agent movement trails (Phase 9.5.3)
    if (movementTrails.length > 0) {
      const trailLayer = g.append("g");

      for (const trail of movementTrails) {
        const fromCoord = coordLookup.get(trail.fromLocationId);
        const toCoord = coordLookup.get(trail.toLocationId);
        if (!fromCoord || !toCoord) continue;
        if (fromCoord.x === toCoord.x && fromCoord.y === toCoord.y) continue;

        const age = currentTick - trail.tick;
        const opacity = Math.max(0.1, 1 - age / 10);
        const color = agentColor(trail.agentId);

        trailLayer
          .append("path")
          .attr("d", routeBezier(fromCoord, toCoord))
          .attr("fill", "none")
          .attr("stroke", color)
          .attr("stroke-width", 1.5)
          .attr("stroke-dasharray", "3,4")
          .attr("opacity", opacity * 0.6)
          .attr("stroke-linecap", "round");
      }
    }

    // Layer 7: Resource heatmap underlays (Phase 9.5.4)
    if (showHeatmap) {
      const heatLayer = g.append("g");

      for (const loc of positionedLocations) {
        const data = heatmapData.get(loc.id);
        if (!data) continue;

        heatLayer
          .append("circle")
          .attr("cx", loc.coord.x)
          .attr("cy", loc.coord.y)
          .attr("r", 35 + data.intensity * 20)
          .attr("fill", data.dominantColor)
          .attr("fill-opacity", 0.12 + data.intensity * 0.15)
          .attr("filter", "url(#heat-glow)");
      }
    }

    // Layer 8: Routes (quadratic Bezier curves, color-coded)
    const routeLayer = g.append("g");

    for (const route of validRoutes) {
      const fromCoord = coordLookup.get(route.from);
      const toCoord = coordLookup.get(route.to);
      if (!fromCoord || !toCoord) continue;

      const routeKey = `${route.from}-${route.to}`;
      const isHovered = hoveredRoute === routeKey;
      const durabilityRatio =
        route.maxDurability > 0 ? route.durability / route.maxDurability : 1;

      // Route path
      routeLayer
        .append("path")
        .attr("d", routeBezier(fromCoord, toCoord))
        .attr("fill", "none")
        .attr("stroke", isHovered ? "#c9d1d9" : getPathColor(route.pathType))
        .attr("stroke-width", isHovered ? getPathWidth(route.pathType) + 1 : getPathWidth(route.pathType))
        .attr("stroke-dasharray", getPathDash(route.pathType))
        .attr("opacity", 0.4 + durabilityRatio * 0.5);

      // Invisible wider hit target for hover
      routeLayer
        .append("path")
        .attr("d", routeBezier(fromCoord, toCoord))
        .attr("fill", "none")
        .attr("stroke", "transparent")
        .attr("stroke-width", 12)
        .attr("cursor", "pointer")
        .on("mouseenter", (event: MouseEvent) => {
          setHoveredRoute(routeKey);
          const container = containerRef.current;
          if (!container) return;
          const rect = container.getBoundingClientRect();
          setMapTooltip({
            x: event.clientX - rect.left,
            y: event.clientY - rect.top,
            title: "Route",
            rows: [
              { label: "Type", value: route.pathType },
              { label: "Cost", value: `${route.cost} ticks` },
              { label: "Durability", value: `${route.durability}/${route.maxDurability}` },
            ],
          });
        })
        .on("mouseleave", () => {
          setHoveredRoute(null);
          setMapTooltip(null);
        });

      // Cost label (show on hover or always for compact routes)
      const mid = routeMidpoint(fromCoord, toCoord);
      if (isHovered) {
        // Background pill for hovered route label
        routeLayer
          .append("rect")
          .attr("x", mid.x - 30)
          .attr("y", mid.y - 11)
          .attr("width", 60)
          .attr("height", 16)
          .attr("rx", 3)
          .attr("fill", "#21262d")
          .attr("stroke", "#30363d")
          .attr("stroke-width", 0.5);

        routeLayer
          .append("text")
          .attr("x", mid.x)
          .attr("y", mid.y + 1)
          .attr("text-anchor", "middle")
          .attr("fill", "#c9d1d9")
          .attr("font-size", "9px")
          .attr("font-family", "var(--font-mono)")
          .text(`${route.cost}t  ${route.pathType}`);
      } else {
        routeLayer
          .append("text")
          .attr("x", mid.x)
          .attr("y", mid.y)
          .attr("text-anchor", "middle")
          .attr("fill", "#4a5568")
          .attr("font-size", "8px")
          .attr("font-family", "var(--font-mono)")
          .text(`${route.cost}t`);
      }
    }

    // Layer 9: Location nodes
    const nodeLayer = g.append("g");

    for (const loc of positionedLocations) {
      drawLocationNode(
        nodeLayer,
        loc,
        loc.id === selectedLocationId,
        showHeatmap,
        onSelectLocation,
        (event: MouseEvent, hoveredLoc: PositionedLocation) => {
          const container = containerRef.current;
          if (!container) return;
          const rect = container.getBoundingClientRect();
          setMapTooltip({
            x: event.clientX - rect.left,
            y: event.clientY - rect.top,
            title: hoveredLoc.name,
            rows: [
              { label: "Region", value: hoveredLoc.region },
              { label: "Type", value: hoveredLoc.location_type },
              { label: "Agents", value: String(hoveredLoc.agentCount) },
            ],
          });
        },
        () => setMapTooltip(null),
      );
    }

    // Layer 10: Agent name labels (Phase 9.5.5)
    if (showLabels) {
      const labelLayer = g.append("g");
      drawAgentLabels(labelLayer, agents, coordLookup, agentCounts);
    }
  }, [
    positionedLocations,
    validRoutes,
    coordLookup,
    movementTrails,
    currentTick,
    showHeatmap,
    heatmapData,
    showLabels,
    agents,
    agentCounts,
    hoveredRoute,
    selectedLocationId,
    onSelectLocation,
  ]);

  // -------------------------------------------------------------------------
  // Effects
  // -------------------------------------------------------------------------

  useEffect(() => {
    renderMap();
  }, [renderMap]);

  // -------------------------------------------------------------------------
  // JSX
  // -------------------------------------------------------------------------

  return (
    <div className="flex-1 min-h-0 flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      {/* Header with toggle controls */}
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>World Map</span>
        <div className="flex items-center gap-md">
          {/* Toggle buttons */}
          <div className="flex items-center gap-xs">
            <button
              className={cn(
                "px-1.5 py-0.5 rounded-sm text-2xs font-mono border cursor-pointer transition-colors duration-100",
                showTrails
                  ? "bg-info/15 text-info border-info/30"
                  : "bg-transparent text-text-muted border-border-secondary hover:text-text-secondary",
              )}
              onClick={() => setShowTrails((v) => !v)}
              title="Toggle movement trails"
            >
              Trails
            </button>
            <button
              className={cn(
                "px-1.5 py-0.5 rounded-sm text-2xs font-mono border cursor-pointer transition-colors duration-100",
                showLabels
                  ? "bg-info/15 text-info border-info/30"
                  : "bg-transparent text-text-muted border-border-secondary hover:text-text-secondary",
              )}
              onClick={() => setShowLabels((v) => !v)}
              title="Toggle agent name labels"
            >
              Names
            </button>
            <button
              className={cn(
                "px-1.5 py-0.5 rounded-sm text-2xs font-mono border cursor-pointer transition-colors duration-100",
                showHeatmap
                  ? "bg-warning/15 text-warning border-warning/30"
                  : "bg-transparent text-text-muted border-border-secondary hover:text-text-secondary",
              )}
              onClick={() => setShowHeatmap((v) => !v)}
              title="Toggle resource heatmap"
            >
              Heatmap
            </button>
          </div>
          <span className="text-xs font-normal normal-case tracking-normal">
            {locations.length} locations / {agents.filter((a) => a.alive).length} agents
            {validRoutes.length > 0 && ` / ${validRoutes.length} routes`}
          </span>
        </div>
      </div>

      <div ref={containerRef} className="flex-1 min-h-0 relative overflow-hidden p-0 bg-ocean">
        <svg ref={svgRef} className="block w-full h-full" />

        {/* Location detail popup (Phase 9.5.2) */}
        {selectedLocationId &&
          locationDetail &&
          locationDetail.location.id === selectedLocationId &&
          selectedLocationCoord &&
          containerRect && (
            <LocationDetailPopup
              locationDetail={locationDetail}
              coord={selectedLocationCoord}
              containerRect={containerRect}
              onClose={() => onSelectLocation(selectedLocationId)}
              onSelectAgent={onSelectAgent}
            />
          )}

        {/* Chart tooltip for map elements */}
        <ChartTooltip data={mapTooltip} />

        {/* Heatmap legend (Phase 9.5.4) */}
        {showHeatmap && <HeatmapLegend />}

        {/* Region legend */}
        <div className="absolute bottom-2 left-2 flex gap-3 text-xs font-mono text-text-secondary">
          {Object.entries(REGION_COLORS).map(([region, color]) => (
            <span key={region} className="flex items-center gap-1">
              <span className="w-2 h-2 rounded-full inline-block" style={{ background: color }} />
              {region}
            </span>
          ))}
        </div>

        {/* Route type legend (when routes exist) */}
        {validRoutes.length > 0 && (
          <div className="absolute bottom-2 right-2 flex gap-3 text-2xs font-mono text-text-muted">
            <span className="flex items-center gap-1">
              <span
                className="w-4 h-0.5 inline-block rounded"
                style={{ background: getPathColor("Highway") }}
              />
              Highway
            </span>
            <span className="flex items-center gap-1">
              <span
                className="w-4 h-0.5 inline-block rounded"
                style={{ background: getPathColor("Road") }}
              />
              Road
            </span>
            <span className="flex items-center gap-1">
              <span
                className="w-4 h-0.5 inline-block rounded border-dashed"
                style={{
                  background: "transparent",
                  borderBottom: `1px dashed ${getPathColor("DirtTrail")}`,
                }}
              />
              Trail
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

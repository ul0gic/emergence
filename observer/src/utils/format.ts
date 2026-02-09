/**
 * Formatting utilities for the Observer Dashboard.
 *
 * Numbers are formatted with commas for thousands, percentages with one
 * decimal, resource names are humanized, and colors are mapped consistently.
 */
import type { EventType, Resource, Season } from "../types/generated/index.ts";

/**
 * Format a number with commas for thousands separator.
 */
export function formatNumber(n: number): string {
  return n.toLocaleString("en-US");
}

/**
 * Format a decimal string (from Rust Decimal) as a float with specified
 * decimal places.
 */
export function formatDecimal(s: string, decimals = 2): string {
  const n = parseFloat(s);
  if (Number.isNaN(n)) return s;
  return n.toFixed(decimals);
}

/**
 * Format a percentage with one decimal place.
 */
export function formatPercent(n: number, total: number): string {
  if (total === 0) return "0.0%";
  return ((n / total) * 100).toFixed(1) + "%";
}

/**
 * Format a Gini coefficient string as a percentage.
 */
export function formatGini(s: string): string {
  const n = parseFloat(s);
  if (Number.isNaN(n)) return s;
  return (n * 100).toFixed(1) + "%";
}

/**
 * Format resource name for display (e.g., "FoodBerry" -> "Food Berry").
 */
export function formatResourceName(r: Resource): string {
  return r.replace(/([A-Z])/g, " $1").trim();
}

/**
 * Get the event category CSS class for an event type.
 */
export function getEventCategory(eventType: EventType): string {
  switch (eventType) {
    case "AgentBorn":
    case "AgentDied":
      return "event-lifecycle";
    case "ResourceGathered":
    case "ResourceConsumed":
    case "TradeCompleted":
    case "TradeFailed":
    case "LedgerAnomaly":
      return "event-economy";
    case "MessageSent":
    case "GroupFormed":
    case "RelationshipChanged":
      return "event-social";
    case "StructureBuilt":
    case "StructureDestroyed":
    case "StructureRepaired":
    case "RouteImproved":
    case "LocationDiscovered":
      return "event-world";
    case "KnowledgeDiscovered":
    case "KnowledgeTaught":
      return "event-knowledge";
    case "TickStart":
    case "TickEnd":
    case "ActionSubmitted":
    case "ActionSucceeded":
    case "ActionRejected":
      return "event-system";
    case "WeatherChanged":
    case "SeasonChanged":
      return "event-environment";
    default:
      return "event-system";
  }
}

/**
 * Get the CSS class for a season badge.
 */
export function getSeasonClass(season: Season): string {
  switch (season) {
    case "Spring":
      return "season-spring";
    case "Summer":
      return "season-summer";
    case "Autumn":
      return "season-autumn";
    case "Winter":
      return "season-winter";
    default:
      return "";
  }
}

/**
 * Get a hex color for a resource type, used in charts.
 */
export function getResourceColor(resource: Resource): string {
  const colors: Record<Resource, string> = {
    Water: "#58a6ff",
    FoodBerry: "#f85149",
    FoodFish: "#79c0ff",
    FoodRoot: "#db6d28",
    FoodMeat: "#f47067",
    FoodFarmed: "#3fb950",
    FoodCooked: "#d29922",
    Wood: "#8b6914",
    Stone: "#8b949e",
    Fiber: "#a5d6a7",
    Clay: "#c9733a",
    Hide: "#795548",
    Ore: "#b0bec5",
    Metal: "#cfd8dc",
    Medicine: "#bc8cff",
    Tool: "#f0c040",
    ToolAdvanced: "#f0d070",
    CurrencyToken: "#ffd700",
    WrittenRecord: "#e0e0e0",
  };
  // eslint-disable-next-line security/detect-object-injection -- resource is typed as Resource enum, not user input
  return colors[resource];
}

/**
 * Truncate a UUID for display (show first 8 chars).
 */
export function truncateId(id: string): string {
  return id.slice(0, 8);
}

/**
 * Format a tick count with unit.
 */
export function formatTick(tick: number): string {
  return `T${formatNumber(tick)}`;
}

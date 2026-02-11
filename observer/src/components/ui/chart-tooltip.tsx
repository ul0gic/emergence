/**
 * ChartTooltip -- Reusable floating tooltip for D3 chart elements.
 *
 * This is a lightweight positioned div approach rather than wrapping
 * every SVG element in a Radix Tooltip. It performs better when there
 * are many interactive elements (bars, paths, nodes) in a single chart.
 *
 * Usage from D3:
 *   1. Render <ChartTooltip ... /> alongside your SVG in the chart container.
 *   2. On mouseover, call setTooltip({ x, y, content }) from your D3 code.
 *   3. On mouseout, call setTooltip(null).
 *
 * The component is controlled -- the parent manages the tooltip state.
 */
import { cn } from "@/lib/utils";

export interface ChartTooltipData {
  /** X position in pixels relative to the chart container. */
  x: number;
  /** Y position in pixels relative to the chart container. */
  y: number;
  /** Content rows to display. Each row is a label/value pair. */
  rows: ChartTooltipRow[];
  /** Optional title displayed at the top of the tooltip. */
  title?: string;
}

export interface ChartTooltipRow {
  label: string;
  value: string;
  /** Optional color swatch shown before the label. */
  color?: string;
}

interface ChartTooltipProps {
  data: ChartTooltipData | null;
  /** Additional classes for the tooltip container. */
  className?: string;
}

/**
 * Floating tooltip positioned absolutely within its parent container.
 * The parent must have `position: relative`.
 */
export function ChartTooltip({ data, className }: ChartTooltipProps) {
  if (!data) return null;

  // Offset the tooltip so it doesn't sit directly under the cursor.
  const offsetX = 12;
  const offsetY = -8;

  return (
    <div
      className={cn(
        "absolute z-50 pointer-events-none rounded-md border border-border-primary bg-bg-elevated px-md py-sm font-mono text-xs text-text-primary shadow-lg whitespace-nowrap",
        className,
      )}
      style={{
        left: `${data.x + offsetX}px`,
        top: `${data.y + offsetY}px`,
      }}
    >
      {data.title && (
        <div className="text-2xs text-text-secondary uppercase tracking-wide font-semibold mb-xs">
          {data.title}
        </div>
      )}
      {data.rows.map((row, i) => (
        <div key={i} className="flex items-center gap-sm leading-relaxed">
          {row.color && (
            <span
              className="w-2 h-2 rounded-sm inline-block shrink-0"
              style={{ background: row.color }}
            />
          )}
          <span className="text-text-secondary">{row.label}:</span>
          <span className="text-text-primary font-semibold">{row.value}</span>
        </div>
      ))}
    </div>
  );
}

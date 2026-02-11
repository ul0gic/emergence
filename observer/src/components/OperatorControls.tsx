/**
 * Operator Controls Panel (Task 6.1.5)
 *
 * Simulation management dashboard: status, speed control, pause/resume,
 * emergency stop, event injection, and health metrics.
 */
import { useCallback, useMemo, useState } from "react";

import {
  useInjectEvent,
  useOperatorStatus,
  usePauseSimulation,
  useResumeSimulation,
  useSetSpeed,
  useStopSimulation,
} from "../hooks/useOperator.ts";
import type { ConnectionStatus } from "../hooks/useWebSocket.ts";
import { cn } from "../lib/utils.ts";
import type { InjectedEventType, OperatorStatus } from "../types/generated/index.ts";
import { formatNumber } from "../utils/format.ts";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SPEED_PRESETS: { label: string; ms: number }[] = [
  { label: "0.5x", ms: 2000 },
  { label: "1x", ms: 1000 },
  { label: "2x", ms: 500 },
  { label: "5x", ms: 200 },
  { label: "10x", ms: 100 },
  { label: "MAX", ms: 10 },
];

const EVENT_TYPES: { value: InjectedEventType; label: string }[] = [
  { value: "natural_disaster", label: "Natural Disaster" },
  { value: "resource_boom", label: "Resource Boom" },
  { value: "plague", label: "Plague" },
  { value: "migration", label: "Migration Pressure" },
  { value: "technology_gift", label: "Technology Gift" },
  { value: "resource_depletion", label: "Resource Depletion" },
];

const REGIONS = ["Highlands", "Central Valley", "Coastal Lowlands"];

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface OperatorControlsProps {
  connectionStatus: ConnectionStatus;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatElapsed(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function formatRemaining(elapsed: number, max: number): string {
  const remaining = Math.max(0, max - elapsed);
  return formatElapsed(remaining);
}

function getEffectiveRate(intervalMs: number): string {
  if (intervalMs <= 0) return "N/A";
  const ticksPerHour = Math.round(3_600_000 / intervalMs);
  return `~${formatNumber(ticksPerHour)} ticks/hr`;
}

function getSimulationStatusLabel(status: OperatorStatus): string {
  if (status.paused) return "Paused";
  if (status.elapsed_seconds >= status.max_real_time_seconds) return "Completed";
  if (status.tick >= status.max_ticks) return "Completed";
  return "Running";
}

function getSimulationStatusClasses(status: OperatorStatus): string {
  const label = getSimulationStatusLabel(status);
  switch (label) {
    case "Running":
      return "bg-success/15 text-success";
    case "Paused":
      return "bg-warning/15 text-warning";
    case "Completed":
      return "bg-info/15 text-info";
    default:
      return "bg-info/15 text-info";
  }
}

function getConnectionDotClasses(connectionStatus: ConnectionStatus): string {
  switch (connectionStatus) {
    case "connected":
      return "bg-success shadow-glow-success";
    case "connecting":
    case "reconnecting":
      return "bg-warning animate-pulse-dot";
    case "disconnected":
      return "bg-danger";
  }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function OperatorControls({
  connectionStatus,
}: OperatorControlsProps) {
  const { data: liveStatus } = useOperatorStatus();
  const status = liveStatus;

  const { execute: pause, loading: pauseLoading } = usePauseSimulation();
  const { execute: resume, loading: resumeLoading } = useResumeSimulation();
  const { execute: setSpeed } = useSetSpeed();
  const { execute: injectEvent, loading: injectLoading } = useInjectEvent();
  const { execute: stop, loading: stopLoading } = useStopSimulation();

  const [showStopConfirm, setShowStopConfirm] = useState(false);
  const [selectedEventType, setSelectedEventType] = useState<InjectedEventType>("natural_disaster");
  const [selectedRegion, setSelectedRegion] = useState<string>("");
  const [showInjectConfirm, setShowInjectConfirm] = useState(false);
  const [lastAction, setLastAction] = useState<string | null>(null);

  const handlePauseResume = useCallback(async () => {
    if (!status) return;
    const result = status.paused ? await resume() : await pause();
    setLastAction(result.message);
  }, [status, pause, resume]);

  const handleSetSpeed = useCallback(
    async (ms: number) => {
      const result = await setSpeed({ tick_interval_ms: ms });
      setLastAction(result.message);
    },
    [setSpeed],
  );

  const handleStop = useCallback(async () => {
    const result = await stop();
    setLastAction(result.message);
    setShowStopConfirm(false);
  }, [stop]);

  const handleInjectEvent = useCallback(async () => {
    const result = await injectEvent({
      event_type: selectedEventType,
      target_region: selectedRegion || undefined,
    });
    setLastAction(result.message);
    setShowInjectConfirm(false);
  }, [injectEvent, selectedEventType, selectedRegion]);

  // Progress calculation.
  const progressPct = useMemo(() => {
    if (!status) return 0;
    if (status.max_real_time_seconds <= 0) return 0;
    return Math.min(100, (status.elapsed_seconds / status.max_real_time_seconds) * 100);
  }, [status]);

  // Active speed preset.
  const activeSpeedPreset = useMemo(() => {
    if (!status) return null;
    return SPEED_PRESETS.find((p) => p.ms === status.tick_interval_ms) ?? null;
  }, [status]);

  if (!status) {
    return (
      <div className="h-full bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
        <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
          Operator Controls
        </div>
        <div className="p-md">
          <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
            Waiting for operator status...
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      {/* Panel header */}
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>Operator Controls</span>
        <span
          className={cn(
            "inline-flex items-center px-1.5 rounded-[10px] text-2xs font-mono font-semibold",
            getSimulationStatusClasses(status),
          )}
        >
          {getSimulationStatusLabel(status)}
        </span>
      </div>

      <div className="flex-1 overflow-y-auto p-md">
        {/* ---------------------------------------------------------------- */}
        {/* Simulation Status Bar */}
        {/* ---------------------------------------------------------------- */}
        <div className="flex gap-sm mb-md">
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Current Tick
            </div>
            <div className="text-lg font-bold text-text-accent font-mono">
              {formatNumber(status.tick)}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Elapsed
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatElapsed(status.elapsed_seconds)}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Remaining
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatRemaining(status.elapsed_seconds, status.max_real_time_seconds)}
            </div>
          </div>
          <div className="flex-1 bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm text-center">
            <div className="text-2xs text-text-secondary font-mono uppercase tracking-wide">
              Population
            </div>
            <div className="text-lg font-bold text-text-primary font-mono">
              {formatNumber(status.agents_alive)}
              <span className="text-2xs text-text-muted font-normal ml-1">
                / {formatNumber(status.agents_alive + status.agents_dead)}
              </span>
            </div>
          </div>
        </div>

        {/* Progress bar */}
        <div className="mb-md">
          <div className="flex justify-between text-2xs font-mono text-text-secondary mb-xs">
            <span>Simulation Progress</span>
            <span>{progressPct.toFixed(1)}%</span>
          </div>
          <div className="h-2 bg-bg-primary rounded-sm overflow-hidden">
            <div
              className="h-full bg-text-accent rounded-sm transition-[width] duration-300 ease-in-out"
              style={{ width: `${progressPct}%` }}
            />
          </div>
        </div>

        {/* ---------------------------------------------------------------- */}
        {/* Speed Control */}
        {/* ---------------------------------------------------------------- */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Speed Control
        </div>
        <div className="mb-md">
          <div className="flex gap-xs mb-sm">
            {SPEED_PRESETS.map((preset) => (
              <button
                key={preset.label}
                className={cn(
                  "px-md py-sm border border-border-primary rounded-sm bg-bg-primary font-mono text-xs cursor-pointer transition-all duration-150",
                  activeSpeedPreset?.ms === preset.ms
                    ? "bg-info/15 border-text-accent text-text-accent"
                    : "text-text-secondary hover:border-text-accent hover:text-text-primary",
                )}
                onClick={() => handleSetSpeed(preset.ms)}
              >
                {preset.label}
              </button>
            ))}
          </div>
          <div className="flex gap-lg text-xs font-mono text-text-secondary">
            <span>
              Interval:{" "}
              <span className="text-text-primary font-semibold">{status.tick_interval_ms}ms</span>
              /tick
            </span>
            <span>
              Rate:{" "}
              <span className="text-text-primary font-semibold">
                {getEffectiveRate(status.tick_interval_ms)}
              </span>
            </span>
          </div>
        </div>

        {/* ---------------------------------------------------------------- */}
        {/* Simulation Control Buttons */}
        {/* ---------------------------------------------------------------- */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Simulation Control
        </div>
        <div className="flex gap-sm mb-md">
          <button
            className={cn(
              "flex-1 px-lg py-sm rounded-sm font-mono text-sm font-semibold cursor-pointer transition-all duration-150 border",
              status.paused
                ? "bg-success/15 border-success text-success hover:bg-success/25"
                : "bg-warning/15 border-warning text-warning hover:bg-warning/25",
              (pauseLoading || resumeLoading) && "opacity-50 cursor-not-allowed",
            )}
            onClick={handlePauseResume}
            disabled={pauseLoading || resumeLoading}
          >
            {status.paused ? "Resume" : "Pause"}
          </button>

          {showStopConfirm ? (
            <div className="flex-1 flex gap-xs">
              <button
                className={cn(
                  "flex-1 px-md py-sm rounded-sm font-mono text-xs font-semibold cursor-pointer transition-all duration-150 border bg-danger/15 border-danger text-danger hover:bg-danger/25",
                  stopLoading && "opacity-50 cursor-not-allowed",
                )}
                onClick={handleStop}
                disabled={stopLoading}
              >
                Confirm Stop
              </button>
              <button
                className="flex-1 px-md py-sm rounded-sm font-mono text-xs font-semibold cursor-pointer transition-all duration-150 border border-border-primary bg-bg-primary text-text-secondary hover:text-text-primary"
                onClick={() => setShowStopConfirm(false)}
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              className="flex-1 px-lg py-sm rounded-sm font-mono text-sm font-semibold cursor-pointer transition-all duration-150 border bg-danger/15 border-danger text-danger hover:bg-danger/25"
              onClick={() => setShowStopConfirm(true)}
            >
              Emergency Stop
            </button>
          )}
        </div>

        {/* ---------------------------------------------------------------- */}
        {/* Event Injection */}
        {/* ---------------------------------------------------------------- */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Event Injection
        </div>
        <div className="mb-md">
          <div className="flex gap-sm mb-sm">
            <div className="flex-1">
              <label className="block text-2xs text-text-muted font-mono mb-xs">Event Type</label>
              <select
                className="w-full px-md py-sm bg-bg-primary border border-border-primary rounded-sm text-text-primary font-mono text-xs outline-none focus:border-text-accent"
                value={selectedEventType}
                onChange={(e) => setSelectedEventType(e.target.value as InjectedEventType)}
              >
                {EVENT_TYPES.map((et) => (
                  <option key={et.value} value={et.value}>
                    {et.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="flex-1">
              <label className="block text-2xs text-text-muted font-mono mb-xs">
                Target Region <span className="text-text-muted">(optional)</span>
              </label>
              <select
                className="w-full px-md py-sm bg-bg-primary border border-border-primary rounded-sm text-text-primary font-mono text-xs outline-none focus:border-text-accent"
                value={selectedRegion}
                onChange={(e) => setSelectedRegion(e.target.value)}
              >
                <option value="">All Regions</option>
                {REGIONS.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {showInjectConfirm ? (
            <div className="flex gap-xs">
              <button
                className={cn(
                  "flex-1 px-md py-sm rounded-sm font-mono text-xs font-semibold cursor-pointer transition-all duration-150 border bg-warning/15 border-warning text-warning hover:bg-warning/25",
                  injectLoading && "opacity-50 cursor-not-allowed",
                )}
                onClick={handleInjectEvent}
                disabled={injectLoading}
              >
                Confirm Injection
              </button>
              <button
                className="flex-1 px-md py-sm rounded-sm font-mono text-xs font-semibold cursor-pointer transition-all duration-150 border border-border-primary bg-bg-primary text-text-secondary hover:text-text-primary"
                onClick={() => setShowInjectConfirm(false)}
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              className="w-full px-md py-sm rounded-sm font-mono text-xs font-semibold cursor-pointer transition-all duration-150 border bg-lifecycle/15 border-lifecycle text-lifecycle hover:bg-lifecycle/25"
              onClick={() => setShowInjectConfirm(true)}
            >
              Inject Event
            </button>
          )}
        </div>

        {/* ---------------------------------------------------------------- */}
        {/* Health Metrics */}
        {/* ---------------------------------------------------------------- */}
        <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mt-md mb-sm pb-xs border-b border-border-secondary">
          Health Metrics
        </div>
        <div className="grid grid-cols-2 gap-sm">
          <div className="bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm">
            <div className="text-2xs text-text-muted font-mono">Avg Tick Duration</div>
            <div className="text-sm font-semibold text-text-primary font-mono">
              {status.elapsed_seconds > 0 && status.tick > 0
                ? `${((status.elapsed_seconds / status.tick) * 1000).toFixed(0)}ms`
                : "N/A"}
            </div>
          </div>
          <div className="bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm">
            <div className="text-2xs text-text-muted font-mono">Uptime</div>
            <div className="text-sm font-semibold text-text-primary font-mono">
              {formatElapsed(status.uptime_seconds)}
            </div>
          </div>
          <div className="bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm">
            <div className="text-2xs text-text-muted font-mono">Era</div>
            <div className="text-sm font-semibold text-text-primary font-mono">{status.era}</div>
          </div>
          <div className="bg-bg-tertiary border border-border-primary rounded-sm px-md py-sm">
            <div className="text-2xs text-text-muted font-mono">WebSocket</div>
            <div className="flex items-center gap-xs">
              <span
                className={cn("w-2 h-2 rounded-full", getConnectionDotClasses(connectionStatus))}
              />
              <span className="text-sm font-semibold text-text-primary font-mono capitalize">
                {connectionStatus}
              </span>
            </div>
          </div>
        </div>

        {/* Last action feedback */}
        {lastAction && (
          <div className="mt-md px-md py-sm bg-bg-tertiary border border-border-primary rounded-sm">
            <div className="text-2xs text-text-muted font-mono">Last Action</div>
            <div className="text-xs text-text-secondary font-mono">{lastAction}</div>
          </div>
        )}
      </div>
    </div>
  );
}

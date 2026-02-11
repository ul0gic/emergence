/**
 * Decision Viewer Panel (Phase 9.3.3-9.3.7) -- "Agent Minds"
 *
 * The killer feature of the Observer: see what agents are THINKING.
 * Shows the full LLM prompt, raw response, parsed action, decision source
 * (rule engine vs LLM vs night cycle), cost tracking, and loop detection.
 *
 * Layout:
 * - Top: Cost dashboard summary bar
 * - Left sidebar: Agent list with decision source indicators
 * - Right panel: Decision stream (newest first) with expandable cards
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useDebounce, useDecisions } from "../hooks/useApi.ts";
import { cn } from "../lib/utils.ts";
import type {
  AgentListItem,
  DecisionRecord,
  DecisionSource,
} from "../types/generated/index.ts";
import { formatNumber, formatTick } from "../utils/format.ts";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DECISION_SOURCE_CONFIG: Record<
  DecisionSource,
  { label: string; shortLabel: string; dotClass: string; badgeClass: string }
> = {
  llm: {
    label: "LLM",
    shortLabel: "LLM",
    dotClass: "bg-success",
    badgeClass: "bg-success/15 text-success border-success/30",
  },
  rule_engine: {
    label: "RULE",
    shortLabel: "RULE",
    dotClass: "bg-info",
    badgeClass: "bg-info/15 text-info border-info/30",
  },
  night_cycle: {
    label: "SLEEP",
    shortLabel: "SLEEP",
    dotClass: "bg-text-muted",
    badgeClass: "bg-text-muted/15 text-text-muted border-text-muted/30",
  },
  timeout: {
    label: "TIMEOUT",
    shortLabel: "TIME",
    dotClass: "bg-warning",
    badgeClass: "bg-warning/15 text-warning border-warning/30",
  },
};

/** Polling interval for fetching fresh decisions (ms). */
const POLL_INTERVAL_MS = 5_000;

/** Threshold for "stuck in loop" detection. */
const STUCK_LOOP_THRESHOLD = 10;

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface DecisionViewerProps {
  agents: AgentListItem[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  if (usd < 1) return `$${usd.toFixed(3)}`;
  return `$${usd.toFixed(2)}`;
}

function formatLatency(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/**
 * Humanize an action_type string: "TradeOffer" -> "Trade Offer".
 */
function humanizeAction(action: string): string {
  return action.replace(/([A-Z])/g, " $1").trim();
}

/**
 * Parse prompt text into sections based on common header markers.
 * Returns an array of { header, body } sections.
 */
function parsePromptSections(
  prompt: string,
): { header: string; body: string }[] {
  // Split on lines that look like section headers:
  // "## System", "# Identity", "=== Perception ===", "--- Memory ---", etc.
  const lines = prompt.split("\n");
  const sections: { header: string; body: string }[] = [];
  let currentHeader = "Prompt";
  let currentBody: string[] = [];

  for (const line of lines) {
    // Detect header patterns
    const headerMatch =
      /^(?:#{1,3}\s+|={3,}\s*|---\s*)(.+?)(?:\s*={3,}|\s*---)?$/.exec(
        line.trim(),
      );
    if (headerMatch && headerMatch[1]) {
      // Save previous section
      if (currentBody.length > 0 || sections.length === 0) {
        sections.push({ header: currentHeader, body: currentBody.join("\n") });
      }
      currentHeader = headerMatch[1].trim();
      currentBody = [];
    } else {
      currentBody.push(line);
    }
  }

  // Push final section
  if (currentBody.length > 0) {
    sections.push({ header: currentHeader, body: currentBody.join("\n") });
  }

  return sections.filter((s) => s.body.trim().length > 0);
}

/**
 * Detect consecutive rule engine overrides per agent.
 * Returns a map of agentId -> { count, rule } for agents currently in a streak.
 */
function detectOverrideStreaks(
  decisions: DecisionRecord[],
): Map<string, { count: number; rule: string }> {
  // Group by agent, ordered by tick descending (decisions should arrive newest first)
  const byAgent = new Map<string, DecisionRecord[]>();
  for (const d of decisions) {
    const existing = byAgent.get(d.agent_id);
    if (existing) {
      existing.push(d);
    } else {
      byAgent.set(d.agent_id, [d]);
    }
  }

  const streaks = new Map<string, { count: number; rule: string }>();

  for (const [agentId, agentDecisions] of byAgent) {
    // Sort by tick descending
    const sorted = [...agentDecisions].sort((a, b) => b.tick - a.tick);

    let streak = 0;
    let streakRule = "";
    for (const d of sorted) {
      if (d.decision_source === "rule_engine") {
        streak++;
        if (streak === 1) {
          streakRule = d.rule_matched ?? "unknown";
        }
      } else {
        break;
      }
    }

    if (streak >= 2) {
      streaks.set(agentId, { count: streak, rule: streakRule });
    }
  }

  return streaks;
}

/**
 * Compute aggregate cost statistics from an array of decision records.
 */
function computeCostStats(decisions: DecisionRecord[]): {
  totalCost: number;
  totalPromptTokens: number;
  totalCompletionTokens: number;
  llmCount: number;
  ruleCount: number;
  sleepCount: number;
  timeoutCount: number;
  totalCount: number;
  avgCostPerTick: number;
} {
  let totalCost = 0;
  let totalPromptTokens = 0;
  let totalCompletionTokens = 0;
  let llmCount = 0;
  let ruleCount = 0;
  let sleepCount = 0;
  let timeoutCount = 0;

  const tickSet = new Set<number>();

  for (const d of decisions) {
    tickSet.add(d.tick);
    switch (d.decision_source) {
      case "llm":
        llmCount++;
        totalCost += d.cost_usd ?? 0;
        totalPromptTokens += d.prompt_tokens ?? 0;
        totalCompletionTokens += d.completion_tokens ?? 0;
        break;
      case "rule_engine":
        ruleCount++;
        break;
      case "night_cycle":
        sleepCount++;
        break;
      case "timeout":
        timeoutCount++;
        break;
    }
  }

  const tickCount = tickSet.size || 1;
  const avgCostPerTick = totalCost / tickCount;

  return {
    totalCost,
    totalPromptTokens,
    totalCompletionTokens,
    llmCount,
    ruleCount,
    sleepCount,
    timeoutCount,
    totalCount: decisions.length,
    avgCostPerTick,
  };
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Cost Dashboard summary bar (Task 9.3.6). */
function CostDashboard({ decisions }: { decisions: DecisionRecord[] }) {
  const stats = useMemo(() => computeCostStats(decisions), [decisions]);
  const totalTokens = stats.totalPromptTokens + stats.totalCompletionTokens;

  // Source breakdown percentages
  const pctLlm =
    stats.totalCount > 0
      ? ((stats.llmCount / stats.totalCount) * 100).toFixed(0)
      : "0";
  const pctRule =
    stats.totalCount > 0
      ? ((stats.ruleCount / stats.totalCount) * 100).toFixed(0)
      : "0";
  const pctSleep =
    stats.totalCount > 0
      ? ((stats.sleepCount / stats.totalCount) * 100).toFixed(0)
      : "0";
  const pctTimeout =
    stats.totalCount > 0
      ? ((stats.timeoutCount / stats.totalCount) * 100).toFixed(0)
      : "0";

  // Bar widths (minimum 2px for visibility when non-zero)
  const barTotal = stats.totalCount || 1;
  const barLlm = stats.llmCount > 0 ? Math.max(2, (stats.llmCount / barTotal) * 100) : 0;
  const barRule = stats.ruleCount > 0 ? Math.max(2, (stats.ruleCount / barTotal) * 100) : 0;
  const barSleep = stats.sleepCount > 0 ? Math.max(2, (stats.sleepCount / barTotal) * 100) : 0;
  const barTimeout = stats.timeoutCount > 0 ? Math.max(2, (stats.timeoutCount / barTotal) * 100) : 0;

  return (
    <div className="flex items-center gap-lg px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-mono flex-wrap">
      {/* Total spend */}
      <div className="flex items-center gap-xs">
        <span className="text-text-muted">SPEND</span>
        <span className="text-success font-semibold">
          {formatCost(stats.totalCost)}
        </span>
      </div>

      {/* Cost per tick */}
      <div className="flex items-center gap-xs">
        <span className="text-text-muted">AVG/TICK</span>
        <span className="text-text-primary">
          {formatCost(stats.avgCostPerTick)}
        </span>
      </div>

      {/* Token counts */}
      <div className="flex items-center gap-xs">
        <span className="text-text-muted">TOKENS</span>
        <span className="text-text-primary">
          {formatNumber(totalTokens)}
        </span>
        <span className="text-text-muted text-2xs">
          ({formatNumber(stats.totalPromptTokens)}p /{" "}
          {formatNumber(stats.totalCompletionTokens)}c)
        </span>
      </div>

      {/* Source breakdown text */}
      <div className="flex items-center gap-sm">
        <span className="text-success">LLM {pctLlm}%</span>
        <span className="text-text-muted">|</span>
        <span className="text-info">Rule {pctRule}%</span>
        <span className="text-text-muted">|</span>
        <span className="text-text-muted">Sleep {pctSleep}%</span>
        {stats.timeoutCount > 0 && (
          <>
            <span className="text-text-muted">|</span>
            <span className="text-warning">Timeout {pctTimeout}%</span>
          </>
        )}
      </div>

      {/* Visual breakdown bar */}
      <div className="flex h-2 flex-1 min-w-[80px] max-w-[200px] rounded-sm overflow-hidden bg-bg-primary">
        {barLlm > 0 && (
          <div
            className="h-full bg-success transition-all duration-300"
            style={{ width: `${barLlm}%` }}
          />
        )}
        {barRule > 0 && (
          <div
            className="h-full bg-info transition-all duration-300"
            style={{ width: `${barRule}%` }}
          />
        )}
        {barSleep > 0 && (
          <div
            className="h-full bg-text-muted transition-all duration-300"
            style={{ width: `${barSleep}%` }}
          />
        )}
        {barTimeout > 0 && (
          <div
            className="h-full bg-warning transition-all duration-300"
            style={{ width: `${barTimeout}%` }}
          />
        )}
      </div>
    </div>
  );
}

/** LLM Prompt Inspector (Task 9.3.5). */
function PromptInspector({ prompt }: { prompt: string }) {
  const sections = useMemo(() => parsePromptSections(prompt), [prompt]);

  return (
    <div className="mt-sm rounded-sm overflow-hidden border border-border-secondary">
      <div className="max-h-[400px] overflow-y-auto bg-bg-primary">
        {sections.length > 1 ? (
          sections.map((section, i) => (
            <div key={i}>
              <div className="px-md py-xs bg-bg-tertiary border-b border-border-secondary font-mono text-2xs text-text-accent uppercase tracking-wide sticky top-0">
                {section.header}
              </div>
              <pre className="px-md py-sm font-mono text-2xs text-text-secondary whitespace-pre-wrap break-words leading-relaxed">
                {section.body.trim()}
              </pre>
            </div>
          ))
        ) : (
          <pre className="px-md py-sm font-mono text-2xs text-text-secondary whitespace-pre-wrap break-words leading-relaxed">
            {prompt}
          </pre>
        )}
      </div>
    </div>
  );
}

/** Decision Detail Card (Task 9.3.4). */
function DecisionCard({
  decision,
  agentName,
  expanded,
  onToggle,
  overrideStreak,
}: {
  decision: DecisionRecord;
  agentName: string;
  expanded: boolean;
  onToggle: () => void;
  overrideStreak: { count: number; rule: string } | null;
}) {
  const [showPrompt, setShowPrompt] = useState(false);
  const [showRawResponse, setShowRawResponse] = useState(false);

  const sourceConfig =
    DECISION_SOURCE_CONFIG[decision.decision_source] ??
    DECISION_SOURCE_CONFIG.timeout;

  const isOverride =
    decision.decision_source === "rule_engine" && overrideStreak !== null;
  const isStuckLoop =
    isOverride && overrideStreak !== null && overrideStreak.count >= STUCK_LOOP_THRESHOLD;

  return (
    <div
      className={cn(
        "mx-sm my-xs rounded-sm border overflow-hidden transition-colors duration-150",
        isStuckLoop
          ? "border-danger/50 bg-danger/5"
          : isOverride
            ? "border-warning/30 bg-warning/5"
            : "border-border-secondary bg-bg-elevated",
      )}
    >
      {/* Compact header -- always visible */}
      <div
        className="flex items-center gap-sm px-md py-sm cursor-pointer"
        onClick={onToggle}
      >
        {/* Tick */}
        <span className="font-mono text-2xs text-text-muted shrink-0 w-14">
          {formatTick(decision.tick)}
        </span>

        {/* Source badge */}
        <span
          className={cn(
            "inline-flex items-center px-1.5 py-px rounded-[10px] text-2xs font-mono font-semibold border shrink-0",
            sourceConfig.badgeClass,
          )}
        >
          {sourceConfig.label}
          {decision.decision_source === "llm" && decision.model && (
            <span className="ml-1 opacity-70 font-normal">
              {decision.model.split("/").pop()}
            </span>
          )}
          {decision.decision_source === "rule_engine" && decision.rule_matched && (
            <span className="ml-1 opacity-70 font-normal">
              {decision.rule_matched}
            </span>
          )}
        </span>

        {/* Override badges (Task 9.3.7) */}
        {isOverride && !isStuckLoop && (
          <span className="inline-flex items-center px-1.5 py-px rounded-[10px] text-2xs font-mono font-semibold border bg-warning/15 text-warning border-warning/30 shrink-0">
            OVERRIDE x{overrideStreak?.count}
          </span>
        )}
        {isStuckLoop && (
          <span className="inline-flex items-center px-1.5 py-px rounded-[10px] text-2xs font-mono font-semibold border bg-danger/15 text-danger border-danger/30 animate-pulse shrink-0">
            STUCK IN LOOP x{overrideStreak?.count}
          </span>
        )}

        {/* Agent name */}
        <span className="text-xs text-text-accent font-semibold shrink-0">
          {agentName}
        </span>

        {/* Action */}
        <span className="text-xs text-text-primary flex-1 truncate">
          {humanizeAction(decision.action_type)}
        </span>

        {/* Quick stats for LLM decisions */}
        {decision.decision_source === "llm" && (
          <div className="flex items-center gap-sm text-2xs font-mono text-text-muted shrink-0">
            {decision.cost_usd !== null && (
              <span className="text-success">
                {formatCost(decision.cost_usd)}
              </span>
            )}
            {decision.latency_ms !== null && (
              <span>{formatLatency(decision.latency_ms)}</span>
            )}
            {decision.prompt_tokens !== null &&
              decision.completion_tokens !== null && (
                <span>
                  {formatNumber(decision.prompt_tokens)}+
                  {formatNumber(decision.completion_tokens)}t
                </span>
              )}
          </div>
        )}

        {/* Expand indicator */}
        <span className="text-text-muted text-2xs shrink-0">
          {expanded ? "-" : "+"}
        </span>
      </div>

      {/* Expanded detail section */}
      {expanded && (
        <div className="px-md pb-md border-t border-border-secondary">
          {/* Header row */}
          <div className="flex items-center gap-md py-sm text-2xs font-mono text-text-muted">
            <span>Tick {formatNumber(decision.tick)}</span>
            <span>Agent: {agentName}</span>
            <span>
              {new Date(decision.created_at).toLocaleTimeString()}
            </span>
          </div>

          {/* Action details */}
          <div className="mb-sm">
            <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mb-xs">
              Action
            </div>
            <div className="text-sm text-text-primary font-semibold">
              {humanizeAction(decision.action_type)}
            </div>
            {decision.action_params &&
              Object.keys(decision.action_params).length > 0 && (
                <pre className="mt-xs px-sm py-xs bg-bg-primary rounded-sm font-mono text-2xs text-text-secondary whitespace-pre-wrap break-all">
                  {JSON.stringify(decision.action_params, null, 2)}
                </pre>
              )}
          </div>

          {/* LLM-specific details (Task 9.3.4) */}
          {decision.decision_source === "llm" && (
            <div className="mb-sm">
              <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mb-xs">
                LLM Details
              </div>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-xs">
                {decision.model && (
                  <div className="px-sm py-xs bg-bg-primary rounded-sm">
                    <div className="text-2xs text-text-muted">Model</div>
                    <div className="text-xs text-text-primary font-mono truncate">
                      {decision.model}
                    </div>
                  </div>
                )}
                {decision.llm_backend && (
                  <div className="px-sm py-xs bg-bg-primary rounded-sm">
                    <div className="text-2xs text-text-muted">Backend</div>
                    <div className="text-xs text-text-primary font-mono">
                      {decision.llm_backend}
                    </div>
                  </div>
                )}
                {decision.prompt_tokens !== null && (
                  <div className="px-sm py-xs bg-bg-primary rounded-sm">
                    <div className="text-2xs text-text-muted">
                      Prompt Tokens
                    </div>
                    <div className="text-xs text-text-primary font-mono">
                      {formatNumber(decision.prompt_tokens)}
                    </div>
                  </div>
                )}
                {decision.completion_tokens !== null && (
                  <div className="px-sm py-xs bg-bg-primary rounded-sm">
                    <div className="text-2xs text-text-muted">
                      Completion Tokens
                    </div>
                    <div className="text-xs text-text-primary font-mono">
                      {formatNumber(decision.completion_tokens)}
                    </div>
                  </div>
                )}
                {decision.cost_usd !== null && (
                  <div className="px-sm py-xs bg-bg-primary rounded-sm">
                    <div className="text-2xs text-text-muted">Cost</div>
                    <div className="text-xs text-success font-mono font-semibold">
                      {formatCost(decision.cost_usd)}
                    </div>
                  </div>
                )}
                {decision.latency_ms !== null && (
                  <div className="px-sm py-xs bg-bg-primary rounded-sm">
                    <div className="text-2xs text-text-muted">Latency</div>
                    <div className="text-xs text-text-primary font-mono">
                      {formatLatency(decision.latency_ms)}
                    </div>
                  </div>
                )}
              </div>

              {/* Raw LLM Response (collapsible) */}
              {decision.raw_llm_response && (
                <div className="mt-sm">
                  <button
                    className="text-2xs font-mono text-text-accent cursor-pointer bg-transparent border-0 p-0 hover:underline"
                    onClick={(e) => {
                      e.stopPropagation();
                      setShowRawResponse(!showRawResponse);
                    }}
                  >
                    {showRawResponse
                      ? "- Hide Raw Response"
                      : "+ View Raw Response"}
                  </button>
                  {showRawResponse && (
                    <pre className="mt-xs px-sm py-sm bg-bg-primary rounded-sm font-mono text-2xs text-text-secondary whitespace-pre-wrap break-words max-h-[300px] overflow-y-auto border border-border-secondary">
                      {decision.raw_llm_response}
                    </pre>
                  )}
                </div>
              )}

              {/* Full Prompt Inspector (Task 9.3.5) */}
              {decision.prompt_sent && (
                <div className="mt-sm">
                  <button
                    className="text-2xs font-mono text-text-accent cursor-pointer bg-transparent border-0 p-0 hover:underline"
                    onClick={(e) => {
                      e.stopPropagation();
                      setShowPrompt(!showPrompt);
                    }}
                  >
                    {showPrompt
                      ? "- Hide Full Prompt"
                      : "+ View Full Prompt"}
                  </button>
                  {showPrompt && (
                    <PromptInspector prompt={decision.prompt_sent} />
                  )}
                </div>
              )}
            </div>
          )}

          {/* Rule Engine details (Task 9.3.4) */}
          {decision.decision_source === "rule_engine" && (
            <div className="mb-sm">
              <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mb-xs">
                Rule Engine Details
              </div>
              <div className="px-sm py-xs bg-bg-primary rounded-sm">
                <div className="text-xs text-info font-mono">
                  Rule matched:{" "}
                  <span className="text-text-primary font-semibold">
                    {decision.rule_matched ?? "unknown"}
                  </span>
                </div>
              </div>
              {isStuckLoop && overrideStreak && (
                <div className="mt-xs px-sm py-xs bg-danger/10 border border-danger/30 rounded-sm">
                  <div className="text-xs text-danger font-mono font-semibold">
                    Agent stuck in rule engine loop for{" "}
                    {overrideStreak.count} consecutive ticks
                  </div>
                  <div className="text-2xs text-danger/70 font-mono mt-xs">
                    Repeating rule: {overrideStreak.rule}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Night cycle details */}
          {decision.decision_source === "night_cycle" && (
            <div className="mb-sm">
              <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mb-xs">
                Night Cycle
              </div>
              <div className="px-sm py-xs bg-bg-primary rounded-sm text-xs text-text-secondary font-mono">
                Agent is sleeping -- no decision needed
              </div>
            </div>
          )}

          {/* Timeout details */}
          {decision.decision_source === "timeout" && (
            <div className="mb-sm">
              <div className="font-mono text-2xs text-text-muted uppercase tracking-widest mb-xs">
                Timeout
              </div>
              <div className="px-sm py-xs bg-warning/10 border border-warning/30 rounded-sm text-xs text-warning font-mono">
                LLM response timed out -- fallback action assigned
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function DecisionViewer({ agents }: DecisionViewerProps) {
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [searchText, setSearchText] = useState("");
  const [expandedKeys, setExpandedKeys] = useState<Set<string>>(new Set());
  const debouncedSearch = useDebounce(searchText, 200);

  // Fetch decisions -- either for selected agent or all
  const { decisions, refetch: refetchDecisions } = useDecisions({
    agentId: selectedAgentId,
    limit: selectedAgentId ? 50 : 200,
  });

  // Poll for new decisions
  const refetchRef = useRef(refetchDecisions);
  refetchRef.current = refetchDecisions;

  useEffect(() => {
    const interval = setInterval(() => {
      refetchRef.current();
    }, POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, []);

  // Refetch when agent selection changes
  useEffect(() => {
    refetchDecisions();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedAgentId]);

  // Agent name map
  const agentNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agents) {
      map.set(a.id, a.name);
    }
    return map;
  }, [agents]);

  // Compute override streaks for loop detection (Task 9.3.7)
  const overrideStreaks = useMemo(
    () => detectOverrideStreaks(decisions),
    [decisions],
  );

  // Build per-agent "last decision source" lookup for the sidebar
  const agentLastSource = useMemo(() => {
    const map = new Map<string, DecisionSource>();
    // Decisions are newest first; take the first one per agent
    for (const d of decisions) {
      if (!map.has(d.agent_id)) {
        map.set(d.agent_id, d.decision_source);
      }
    }
    return map;
  }, [decisions]);

  // Filtered agent list for sidebar
  const filteredAgents = useMemo(() => {
    if (!debouncedSearch) return agents;
    const q = debouncedSearch.toLowerCase();
    return agents.filter(
      (a) =>
        a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q),
    );
  }, [agents, debouncedSearch]);

  // Sorted decisions (newest first)
  const sortedDecisions = useMemo(
    () => [...decisions].sort((a, b) => b.tick - a.tick || b.created_at.localeCompare(a.created_at)),
    [decisions],
  );

  const toggleExpand = useCallback((key: string) => {
    setExpandedKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  const handleSelectAgent = useCallback((agentId: string | null) => {
    setSelectedAgentId(agentId);
    setExpandedKeys(new Set());
  }, []);

  return (
    <div className="h-full flex flex-col bg-bg-secondary border border-border-primary rounded-md overflow-hidden">
      {/* Panel header */}
      <div className="flex items-center justify-between px-md py-sm bg-bg-tertiary border-b border-border-primary text-xs font-semibold text-text-secondary font-mono uppercase tracking-wide">
        <span>Agent Minds</span>
        <div className="flex items-center gap-sm text-xs font-normal">
          <span>{formatNumber(decisions.length)} decisions</span>
        </div>
      </div>

      {/* Cost dashboard (Task 9.3.6) */}
      <CostDashboard decisions={decisions} />

      {/* Main content: sidebar + decision stream */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left sidebar: Agent list */}
        <div className="w-60 border-r border-border-primary flex flex-col shrink-0">
          <div className="p-sm">
            <input
              className="w-full px-md py-sm bg-bg-primary border border-border-primary rounded-sm text-text-primary font-mono text-xs outline-none focus:border-text-accent placeholder:text-text-muted"
              placeholder="Search agents..."
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
            />
          </div>

          <ul className="list-none overflow-y-auto flex-1">
            {/* "All Agents" option */}
            <li
              className={cn(
                "flex items-center justify-between px-md py-sm border-b border-border-secondary cursor-pointer transition-colors duration-100 text-xs hover:bg-bg-tertiary",
                selectedAgentId === null &&
                  "bg-bg-elevated border-l-2 border-l-text-accent",
              )}
              onClick={() => handleSelectAgent(null)}
            >
              <div>
                <div className="font-semibold text-sm">All Agents</div>
                <div className="text-2xs text-text-muted font-mono">
                  {formatNumber(decisions.length)} decisions
                </div>
              </div>
            </li>

            {filteredAgents.map((agent) => {
              const lastSource = agentLastSource.get(agent.id);
              const streak = overrideStreaks.get(agent.id);
              const isStuck =
                streak !== undefined && streak.count >= STUCK_LOOP_THRESHOLD;

              return (
                <li
                  key={agent.id}
                  className={cn(
                    "flex items-center justify-between px-md py-sm border-b border-border-secondary cursor-pointer transition-colors duration-100 text-xs hover:bg-bg-tertiary",
                    selectedAgentId === agent.id &&
                      "bg-bg-elevated border-l-2 border-l-text-accent",
                  )}
                  onClick={() => handleSelectAgent(agent.id)}
                >
                  <div className="flex items-center gap-sm min-w-0">
                    {/* Decision source dot */}
                    <span
                      className={cn(
                        "w-2 h-2 rounded-full shrink-0",
                        lastSource
                          ? DECISION_SOURCE_CONFIG[lastSource]?.dotClass ??
                              "bg-text-muted"
                          : "bg-border-secondary",
                      )}
                    />
                    <div className="min-w-0">
                      <div className="font-semibold text-sm truncate">
                        {agent.name}
                      </div>
                      {!agent.alive && (
                        <span className="text-2xs text-danger font-mono">
                          dead
                        </span>
                      )}
                    </div>
                  </div>

                  {/* Streak warning badges */}
                  <div className="flex items-center gap-xs shrink-0">
                    {isStuck && (
                      <span className="inline-flex items-center px-1 rounded-[10px] text-2xs font-mono font-semibold bg-danger/15 text-danger">
                        LOOP
                      </span>
                    )}
                    {streak && !isStuck && (
                      <span className="inline-flex items-center px-1 rounded-[10px] text-2xs font-mono text-warning">
                        x{streak.count}
                      </span>
                    )}
                  </div>
                </li>
              );
            })}

            {filteredAgents.length === 0 && (
              <li className="flex items-center justify-center px-md py-sm text-xs text-text-muted cursor-default">
                No agents found
              </li>
            )}
          </ul>
        </div>

        {/* Right panel: Decision stream */}
        <div className="flex-1 overflow-y-auto">
          {sortedDecisions.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-xl text-text-muted font-mono text-xs text-center min-h-[120px]">
              {selectedAgentId
                ? "No decisions recorded for this agent"
                : "No decisions recorded yet"}
              <div className="mt-sm text-2xs">
                Decisions will appear here as agents make choices each tick
              </div>
            </div>
          ) : (
            sortedDecisions.map((decision, index) => {
              const key = `${decision.agent_id}-${decision.tick}-${index}`;
              const name =
                agentNameMap.get(decision.agent_id) ??
                decision.agent_id.slice(0, 8);
              const streak = overrideStreaks.get(decision.agent_id) ?? null;

              // Only pass streak info for the most recent decision per agent
              // to avoid badge spam on older cards
              const isLatestForAgent =
                sortedDecisions.findIndex(
                  (d) => d.agent_id === decision.agent_id,
                ) === index;

              return (
                <DecisionCard
                  key={key}
                  decision={decision}
                  agentName={name}
                  expanded={expandedKeys.has(key)}
                  onToggle={() => toggleExpand(key)}
                  overrideStreak={
                    isLatestForAgent &&
                    decision.decision_source === "rule_engine"
                      ? streak
                      : null
                  }
                />
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}

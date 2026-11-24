export function timeAgo(milliseconds) {
  const seconds = Math.max(0, Math.round((Date.now() - milliseconds) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h ago`;
  return `${Math.round(seconds / 86400)}d ago`;
}

export function countOf(count, noun) {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

/** How long after `earlier` `later` came, like "16 seconds later". Both are ISO timestamps; null when either is missing. */
export function timeLater(earlier, later) {
  if (!earlier || !later) return null;
  const seconds = Math.round((Date.parse(later) - Date.parse(earlier)) / 1000);
  if (Number.isNaN(seconds) || seconds < 0) return null;
  if (seconds < 1) return "under a second later";
  if (seconds < 60) return `${countOf(seconds, "second")} later`;
  if (seconds < 3600) return `${countOf(Math.round(seconds / 60), "minute")} later`;
  if (seconds < 86400) return `${countOf(Math.round(seconds / 3600), "hour")} later`;
  return `${countOf(Math.round(seconds / 86400), "day")} later`;
}

/** A token count in short form, like "80k" or "1M". */
export function tokenCount(tokens) {
  if (tokens >= 1_000_000) return `${Number((tokens / 1_000_000).toFixed(1))}M`;
  if (tokens >= 1000) return `${Math.round(tokens / 1000)}k`;
  return String(tokens);
}

/** How full an agent's context window is in short form, like "9%", or "80k" when the window's size is unknown. */
export function contextUsagePercent({ used_tokens, percent_used }) {
  return percent_used == null ? tokenCount(used_tokens) : `${percent_used}%`;
}

/** How full an agent's context window is, like "80k of 1M tokens (9%)", from a `ContextUsage`. */
export function contextUsageText({ used_tokens, window_tokens, percent_used }) {
  if (window_tokens == null) return `${tokenCount(used_tokens)} tokens`;
  return `${tokenCount(used_tokens)} of ${tokenCount(window_tokens)} tokens (${percent_used}%)`;
}

/** The agents' names, by `AgentKind`. */
export const AGENT_NAMES = { claude_code: "Claude Code", cursor_agent: "Cursor", codex: "Codex" };

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

/** The agents' names, by `AgentKind`. */
export const AGENT_NAMES = { claude_code: "Claude Code", cursor_agent: "Cursor" };

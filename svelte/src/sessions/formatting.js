import { msgid, ngettext, t } from "ttag";

/** How long ago, in short form, like "5m ago". */
export function timeAgo(milliseconds) {
  const seconds = Math.max(0, Math.round((Date.now() - milliseconds) / 1000));
  if (seconds < 60) return t`${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (seconds < 3600) return t`${minutes}m ago`;
  const hours = Math.round(seconds / 3600);
  if (seconds < 86400) return t`${hours}h ago`;
  const days = Math.round(seconds / 86400);
  return t`${days}d ago`;
}

/** How long after `earlier` `later` came, like "16 seconds later". Both are ISO timestamps; null when either is missing. */
export function timeLater(earlier, later) {
  if (!earlier || !later) return null;
  const seconds = Math.round((Date.parse(later) - Date.parse(earlier)) / 1000);
  if (Number.isNaN(seconds) || seconds < 0) return null;
  if (seconds < 1) return t`under a second later`;
  if (seconds < 60) return ngettext(msgid`${seconds} second later`, `${seconds} seconds later`, seconds);
  const minutes = Math.round(seconds / 60);
  if (seconds < 3600) return ngettext(msgid`${minutes} minute later`, `${minutes} minutes later`, minutes);
  const hours = Math.round(seconds / 3600);
  if (seconds < 86400) return ngettext(msgid`${hours} hour later`, `${hours} hours later`, hours);
  const days = Math.round(seconds / 86400);
  return ngettext(msgid`${days} day later`, `${days} days later`, days);
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
  const used = tokenCount(used_tokens);
  if (window_tokens == null) return t`${used} tokens`;
  const window = tokenCount(window_tokens);
  return t`${used} of ${window} tokens (${percent_used}%)`;
}

/** The agents' names, by `AgentKind`. */
export const AGENT_NAMES = { claude_code: "Claude Code", cursor_agent: "Cursor", codex: "Codex" };

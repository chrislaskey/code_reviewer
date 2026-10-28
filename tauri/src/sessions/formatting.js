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

<script>
  import ClaudeCodePill from "./ClaudeCodePill.svelte";
  import ShortPath from "./ShortPath.svelte";

  let { session, homeDirectory } = $props();

  let location = $derived.by(() => {
    if (!session.tmux_location) return "not in tmux";
    return session.tmux_pane_found
      ? `tmux ${session.tmux_location}`
      : `tmux ${session.tmux_location} (another tmux server)`;
  });

  function timeAgo(milliseconds) {
    if (!milliseconds) return "";
    const seconds = Math.max(0, Math.round((Date.now() - milliseconds) / 1000));
    if (seconds < 60) return `${seconds}s ago`;
    if (seconds < 3600) return `${Math.round(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.round(seconds / 3600)}h ago`;
    return `${Math.round(seconds / 86400)}d ago`;
  }
</script>

<li>
  <strong title="Session {session.session_id ?? '?'}">{session.name ?? `pid ${session.pid}`}</strong>
  <ClaudeCodePill {session} />
  <div class="details">
    <div class="muted">{location} · v{session.version ?? "?"} · pid {session.pid}</div>
    <ShortPath path={session.cwd ?? ""} {homeDirectory} />
  </div>
  <span class="updated" title="Status last changed">{timeAgo(session.status_updated_at)}</span>
</li>

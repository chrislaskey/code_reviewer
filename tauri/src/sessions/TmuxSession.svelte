<!-- One of the app's tmux sessions: its name, the Claude Code status in it,
     the ways to get to it (the attach command to copy, or Open), its git
     status, the Review and Trace menus into those features and the Complete
     menu to finish it. -->
<script>
  import Button from "../lib/components/Button.svelte";
  import { reviewShowUrl, traceShowUrl } from "../lib/routes.js";
  import ChangesMenu from "./ChangesMenu.svelte";
  import ClaudeCodePill from "./ClaudeCodePill.svelte";
  import CompleteMenu from "./CompleteMenu.svelte";
  import GitStatus from "./GitStatus.svelte";
  import Pill from "./Pill.svelte";
  import ShortPath from "./ShortPath.svelte";

  let { session, homeDirectory, mainBranch, onOpen, onCopy, onMerge, onClose } = $props();

  let attached = $derived(session.attached_clients > 0);
  let attachCommand = $derived(`tmux attach -t ${session.name}`);
  let attachCommandElement;

  let claudeCodeDetails = $derived.by(() => {
    const claudeCode = session.claude_code;
    if (!claudeCode) return "";
    const parts = [];
    if (claudeCode.version) parts.push(`v${claudeCode.version}`);
    parts.push(`pid ${claudeCode.pid}`);
    if (claudeCode.status_updated_at) parts.push(`status changed ${timeAgo(claudeCode.status_updated_at)}`);
    return parts.join(" · ");
  });

  function timeAgo(milliseconds) {
    const seconds = Math.max(0, Math.round((Date.now() - milliseconds) / 1000));
    if (seconds < 60) return `${seconds}s ago`;
    if (seconds < 3600) return `${Math.round(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.round(seconds / 3600)}h ago`;
    return `${Math.round(seconds / 86400)}d ago`;
  }
</script>

<div class="panel">
  <div class="session-header">
    <span class="name">{session.name}</span>
    {#if session.claude_code}
      <ClaudeCodePill session={session.claude_code} />
    {:else}
      <Pill text="no claude" title="No Claude Code is running in the session" />
    {/if}
    <Pill text={attached ? "attached" : "detached"} kind={attached ? "pill-good" : ""} />
    <div class="attach">
      <code bind:this={attachCommandElement} title="Paste into any terminal to attach">{attachCommand}</code>
      <Button class="btn btn-primary btn-soft btn-sm" onclick={() => onCopy(attachCommand, attachCommandElement)}>Copy</Button>
      <Button class="btn btn-primary btn-sm" onclick={() => onOpen(session.name)}>Open</Button>
      {#if session.directory}
        <ChangesMenu label="Review" title="Open the changes in Reviews" directory={session.directory} git={session.git} {mainBranch} urlFor={reviewShowUrl} />
        <ChangesMenu label="Trace" title="Trace a test through the changes in Traces" directory={session.directory} git={session.git} {mainBranch} urlFor={traceShowUrl} />
      {/if}
      <CompleteMenu name={session.name} git={session.git} {mainBranch} {onMerge} {onClose} />
    </div>
  </div>
  <div class="session-details">
    <ShortPath path={session.directory ?? ""} {homeDirectory} />
    {#if session.git}
      <GitStatus git={session.git} />
    {/if}
    <span class="muted">
      {session.window_count} {session.window_count === 1 ? "window" : "windows"}{claudeCodeDetails ? ` · ${claudeCodeDetails}` : ""}
    </span>
  </div>
</div>

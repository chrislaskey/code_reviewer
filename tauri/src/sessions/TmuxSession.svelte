<script>
  import { reviewShowUrl, traceShowUrl } from "../lib/routes.js";
  import Badge from "./Badge.svelte";
  import ChangesMenu from "./ChangesMenu.svelte";
  import ClaudeCodeBadge from "./ClaudeCodeBadge.svelte";
  import GitMenu from "./GitMenu.svelte";
  import OpenMenu from "./OpenMenu.svelte";
  import RepositoryName from "./RepositoryName.svelte";

  let { session, mainBranch, onOpen, onOpenInBrowser, onCopy, onMerge, onClose } = $props();

  let attached = $derived(session.attached_clients > 0);
  let working = $derived(session.claude_code?.status === "busy");
</script>

<div class="panel" class:working>
  <div class="session-header">
    <span class="session-title">
      <span class="name">{session.name}</span>
      <RepositoryName path={session.directory ?? ""} git={session.git} />
    </span>
    {#if session.claude_code}
      <ClaudeCodeBadge session={session.claude_code} />
    {:else}
      <Badge text="no claude" title="No Claude Code is running in the session" />
    {/if}
    {#if !attached}
      <Badge text="detached" title="No terminal window is attached to the session, so Open starts a new one" />
    {/if}
    <div class="session-actions">
      <OpenMenu name={session.name} port={session.port} {onOpen} {onOpenInBrowser} {onCopy} />
      {#if session.directory}
        <ChangesMenu label="Review" title="Open the changes in Reviews" directory={session.directory} git={session.git} {mainBranch} urlFor={reviewShowUrl} />
        <ChangesMenu label="Trace" title="Trace a test through the changes in Traces" directory={session.directory} git={session.git} {mainBranch} urlFor={traceShowUrl} />
      {/if}
      <GitMenu name={session.name} git={session.git} {mainBranch} {onMerge} {onClose} />
    </div>
  </div>
</div>

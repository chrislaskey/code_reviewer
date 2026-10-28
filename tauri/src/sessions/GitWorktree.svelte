<!-- One of the app's git worktrees with no tmux session running in it, as
     after a restart, with its git status, the way to start a session again,
     the Review and Trace menus into those features and the Complete menu to
     finish it. -->
<script>
  import Button from "../lib/components/Button.svelte";
  import { reviewShowUrl, traceShowUrl } from "../lib/routes.js";
  import ChangesMenu from "./ChangesMenu.svelte";
  import CompleteMenu from "./CompleteMenu.svelte";
  import GitStatus from "./GitStatus.svelte";
  import Pill from "./Pill.svelte";
  import ShortPath from "./ShortPath.svelte";

  let { worktree, homeDirectory, mainBranch, onContinue, onMerge, onClose } = $props();
</script>

<div class="panel">
  <div class="session-header">
    <span class="name">{worktree.name}</span>
    <Pill text="no session" title="No tmux session is running in the worktree" />
    <div class="attach">
      <Button class="btn btn-primary btn-sm" onclick={() => onContinue(worktree.name)}>Continue</Button>
      <ChangesMenu label="Review" title="Open the changes in Reviews" directory={worktree.path} git={worktree.git} {mainBranch} urlFor={reviewShowUrl} />
      <ChangesMenu label="Trace" title="Trace a test through the changes in Traces" directory={worktree.path} git={worktree.git} {mainBranch} urlFor={traceShowUrl} />
      <CompleteMenu name={worktree.name} git={worktree.git} {mainBranch} {onMerge} {onClose} />
    </div>
  </div>
  <div class="session-details">
    <ShortPath path={worktree.path} {homeDirectory} />
    {#if worktree.git}
      <GitStatus git={worktree.git} />
    {:else}
      <span class="muted">{worktree.branch ? `branch ${worktree.branch}` : "detached HEAD"}</span>
    {/if}
  </div>
</div>

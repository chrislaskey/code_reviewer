<script>
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";
  import { reviewShowUrl, traceShowUrl } from "../lib/routes.js";
  import Badge from "./Badge.svelte";
  import ChangesMenu from "./ChangesMenu.svelte";
  import GitMenu from "./GitMenu.svelte";
  import RepositoryName from "./RepositoryName.svelte";

  let { worktree, mainBranch, onContinue, onMerge, onClose } = $props();
</script>

<div class="panel">
  <div class="session-header">
    <span class="session-title">
      <span class="name no-session">{worktree.name}</span>
      <RepositoryName path={worktree.path} git={worktree.git} />
    </span>
    <Badge text="no session" kind="badge-ghost" title="No tmux session is running in the worktree" />
    <div class="session-actions">
      <ChoiceMenu label="Open" title="Start a session in the worktree">
        {#snippet children({ close })}
          <MenuChoice
            label="Open new tmux session"
            explanation="Starts a tmux session in the worktree, opens it in the terminal app and types"
            onclick={() => {
              close();
              onContinue(worktree.name);
            }}
          >
            <code class="whitespace-nowrap">claude --continue</code>
          </MenuChoice>
        {/snippet}
      </ChoiceMenu>
      <ChangesMenu label="Review" title="Open the changes in Reviews" directory={worktree.path} git={worktree.git} {mainBranch} urlFor={reviewShowUrl} />
      <ChangesMenu label="Trace" title="Trace a test through the changes in Traces" directory={worktree.path} git={worktree.git} {mainBranch} urlFor={traceShowUrl} />
      <GitMenu name={worktree.name} git={worktree.git} {mainBranch} {onMerge} {onClose} />
    </div>
  </div>
</div>

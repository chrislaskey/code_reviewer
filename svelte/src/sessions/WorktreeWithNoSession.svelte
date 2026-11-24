<!-- One of the app's worktrees with no tmux session running, on the
     sessions page, with the one action of starting a session in it. -->
<script>
  import Button from "../lib/components/Button.svelte";
  import Badge from "./Badge.svelte";

  let { worktree, onContinue } = $props();

  let repositoryName = $derived(worktree.repository.split("/").filter(Boolean).pop());
</script>

<div class="raised-panel">
  <div class="session-header">
    <span class="session-title">
      <span class="name no-session">{worktree.name}</span>
      <span class="repository-name" title={worktree.path}>{repositoryName}</span>
    </span>
    <Badge text="no session" kind="badge-ghost" title="No tmux session is running in the worktree" />
    <div class="session-actions">
      <Button
        class="btn btn-ghost max-sm:btn-sm"
        title="Starts a tmux session in the worktree, opens it in the terminal app and resumes the agent last chatting there"
        onclick={() => onContinue(worktree.name)}
      >
        Open new tmux session
      </Button>
    </div>
  </div>
</div>

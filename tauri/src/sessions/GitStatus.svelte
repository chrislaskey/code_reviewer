<!-- Like a shell prompt: branch ✔ staged ✘ unstaged ✂ untracked. -->
<script>
  let { git } = $props();

  let branch = $derived(git.branch ?? `@${git.commit ?? "?"}`);
  let branchTitle = $derived(
    git.branch ? `Branch ${git.branch}${git.upstream ? `, tracking ${git.upstream}` : ""}` : "Detached HEAD",
  );
  let counts = $derived(
    [
      [git.staged, "git-staged", "✔", "staged"],
      [git.unstaged, "git-unstaged", "✘", "unstaged"],
      [git.untracked, "git-untracked", "✂", "untracked"],
      [git.conflicted, "git-conflicted", "⚠", "conflicted"],
    ].filter(([count]) => count > 0),
  );
</script>

<div class="git">
  <span class="git-branch" class:dirty={git.dirty} title={branchTitle}>{branch}</span>
  {#each counts as [count, className, symbol, label] (className)}
    <span class={className} title="{count} {label}">{symbol}{count}</span>
  {/each}
  {#if git.ahead > 0 || git.behind > 0}
    <span class="git-sync" title="{git.ahead} ahead of, {git.behind} behind {git.upstream}">
      {git.ahead > 0 ? `↑${git.ahead}` : ""}{git.behind > 0 ? `↓${git.behind}` : ""}
    </span>
  {/if}
</div>

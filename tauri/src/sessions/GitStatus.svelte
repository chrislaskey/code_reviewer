<!-- A git status line like the user's zsh prompt: the repository, the branch
     (yellow when there are changes, green when clean), then ✔ staged,
     ✘ unstaged and ✂ untracked, each with its count when more than one.
     Conflicts and commits ahead or behind the upstream follow, as the prompt
     has no room for them but the Git menu acts on them. -->
<script>
  let { git } = $props();

  let branch = $derived(git.branch ?? git.commit ?? "no commits");
  let branchTitle = $derived(
    git.branch
      ? `Branch ${git.branch}${git.upstream ? `, tracking ${git.upstream}` : ""}`
      : `Detached HEAD${git.commit ? ` at ${git.commit}` : ""}`,
  );
  let changes = $derived(
    [
      [git.staged, "git-staged", "✔", "staged"],
      [git.unstaged, "git-unstaged", "✘", "unstaged"],
      [git.untracked, "git-untracked", "✂", "untracked"],
      [git.conflicted, "git-conflicted", "⚠", "conflicted"],
    ].filter(([count]) => count > 0),
  );
</script>

<div class="git-status-line">
  {#if git.repository_name}<span class="git-repository">{git.repository_name}</span>{/if}
  <span class="git-branch" class:dirty={git.dirty} title={branchTitle}>{branch}</span>
  {#each changes as [count, className, symbol, label] (className)}
    <span class={className} title="{count} {label}">{count > 1 ? `${symbol} ${count}` : symbol}</span>
  {/each}
  {#if git.ahead > 0 || git.behind > 0}
    <span class="git-sync" title="{git.ahead} ahead of, {git.behind} behind {git.upstream}">
      {git.ahead > 0 ? `↑${git.ahead}` : ""}{git.behind > 0 ? `↓${git.behind}` : ""}
    </span>
  {/if}
</div>

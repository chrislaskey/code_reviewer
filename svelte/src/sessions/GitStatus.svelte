<!-- A git status line like the user's zsh prompt: the repository, the branch
     (yellow when there are changes, green when clean), then ✔ staged,
     ✘ unstaged and ✂ untracked, each with its count when more than one.
     Conflicts and commits ahead or behind the upstream follow, as the prompt
     has no room for them but the Git menu acts on them. -->
<script>
  import { msgid, ngettext, t } from "ttag";

  let { git } = $props();

  let branch = $derived(git.branch ?? git.commit ?? t`no commits`);
  let branchTitle = $derived.by(() => {
    const { branch, upstream, commit } = git;
    if (branch) return upstream ? t`Branch ${branch}, tracking ${upstream}` : t`Branch ${branch}`;
    return commit ? t`Detached HEAD at ${commit}` : t`Detached HEAD`;
  });
  let changes = $derived(
    [
      [git.staged, "git-staged", "✔", ngettext(msgid`${git.staged} staged`, `${git.staged} staged`, git.staged)],
      [git.unstaged, "git-unstaged", "✘", ngettext(msgid`${git.unstaged} unstaged`, `${git.unstaged} unstaged`, git.unstaged)],
      [git.untracked, "git-untracked", "✂", ngettext(msgid`${git.untracked} untracked`, `${git.untracked} untracked`, git.untracked)],
      [git.conflicted, "git-conflicted", "⚠", ngettext(msgid`${git.conflicted} conflicted`, `${git.conflicted} conflicted`, git.conflicted)],
    ].filter(([count]) => count > 0),
  );
</script>

<div class="git-status-line">
  {#if git.repository_name}<span class="git-repository">{git.repository_name}</span>{/if}
  <span class="git-branch" class:dirty={git.dirty} title={branchTitle}>{branch}</span>
  {#each changes as [count, className, symbol, title] (className)}
    <span class={className} {title}>{count > 1 ? `${symbol} ${count}` : symbol}</span>
  {/each}
  {#if git.ahead > 0 || git.behind > 0}
    <span class="git-sync" title={t`${git.ahead} ahead of, ${git.behind} behind ${git.upstream}`}>
      {git.ahead > 0 ? `↑${git.ahead}` : ""}{git.behind > 0 ? `↓${git.behind}` : ""}
    </span>
  {/if}
</div>

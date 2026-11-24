<script>
  import { msgid, ngettext, t } from "ttag";
  import ChoiceMenu from "../lib/code_reviewer/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/code_reviewer/components/MenuChoice.svelte";

  let { name, git, mainBranch, onMerge, onClose, class: class_ = undefined } = $props();

  const CHANGES = [
    ["conflicted", t`Conflicted`],
    ["unstaged", t`Unstaged`],
    ["untracked", t`Untracked`],
    ["staged", t`Staged`],
  ];

  /** `3 unstaged changes`, for the most important kind of change. */
  function changeCount(key, count) {
    if (key === "conflicted") return ngettext(msgid`${count} conflicted file`, `${count} conflicted files`, count);
    if (key === "unstaged") return ngettext(msgid`${count} unstaged change`, `${count} unstaged changes`, count);
    if (key === "untracked") return ngettext(msgid`${count} untracked file`, `${count} untracked files`, count);
    if (key === "staged") return ngettext(msgid`${count} staged change`, `${count} staged changes`, count);
    return null;
  }

  let dirty = $derived(git?.dirty ?? false);
  let summary = $derived.by(() => {
    if (!git) return t`no git status`;
    return changeCount(git.most_important_change, git[git.most_important_change]) ?? t`no changes`;
  });
  let branch = $derived.by(() => {
    if (git?.branch) return git.branch;
    const commit = git?.commit;
    return commit ? t`detached at ${commit}` : name;
  });
  let baseBranch = $derived(git?.base_branch && git.base_branch !== mainBranch ? git.base_branch : null);
  let mergeBranch = $derived(baseBranch ?? mainBranch);
</script>

<ChoiceMenu label={t`Git`} title={t`Git: ${summary}`} class={class_}>
  {#snippet children({ close })}
    {#if dirty}
      <li class="menu-title whitespace-normal font-normal text-base-content/70">
        <p class="git-summary">{summary}</p>
        <dl class="git-details">
          <dt>{t`Branch`}</dt>
          <dd>{branch}</dd>
          {#if git.upstream}
            <dt>{t`Upstream`}</dt>
            <dd>{git.upstream}</dd>
            <dt>{t`Commits`}</dt>
            <dd>{t`${git.ahead} ahead, ${git.behind} behind`}</dd>
          {/if}
          {#each CHANGES as [key, label] (key)}
            <dt class={{ [`git-${key}`]: git[key] > 0 }}>{label}</dt>
            <dd>{git[key]}</dd>
          {/each}
        </dl>
        <p class="git-hint">
          {t`Commit or remove the changes before turning it into a branch or merging it into ${mergeBranch}.`}
        </p>
      </li>
    {:else}
      <MenuChoice
        label={t`Turn into branch`}
        explanation={t`Removes the worktree and closes the session but keeps the branch ${branch}, to check out, merge or delete later from the main checkout.`}
        onclick={() => {
          close();
          onClose(name);
        }}
      />
      {#if baseBranch}
        <MenuChoice
          label={t`Merge into ${baseBranch}`}
          explanation={t`Fast-forwards ${baseBranch}, the branch ${branch} was created from, with ${branch}, then removes the worktree, deletes the branch and closes the session. The work lives on in ${baseBranch}.`}
          onclick={() => {
            close();
            onMerge(name, baseBranch);
          }}
        />
      {/if}
      <MenuChoice
        label={t`Merge into ${mainBranch}`}
        explanation={baseBranch
          ? t`Fast-forwards ${mainBranch} with ${branch}, which brings ${baseBranch}'s commits too, then removes the worktree, deletes the branch and closes the session. The work lives on in ${mainBranch}.`
          : t`Fast-forwards ${mainBranch} with ${branch}, then removes the worktree, deletes the branch and closes the session. The work lives on in ${mainBranch}.`}
        onclick={() => {
          close();
          onMerge(name, mainBranch);
        }}
      />
    {/if}
  {/snippet}
</ChoiceMenu>

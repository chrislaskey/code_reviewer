<script>
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";
  import { countOf } from "./formatting.js";

  let { name, git, mainBranch, onMerge, onClose } = $props();

  const CHANGES = [
    ["conflicted", "conflicted file", "Conflicted"],
    ["unstaged", "unstaged change", "Unstaged"],
    ["untracked", "untracked file", "Untracked"],
    ["staged", "staged change", "Staged"],
  ];

  let dirty = $derived(git?.dirty ?? false);
  let summary = $derived.by(() => {
    if (!git) return "no git status";
    const noun = CHANGES.find(([key]) => key === git.most_important_change)?.[1];
    if (!noun) return "no changes";
    return countOf(git[git.most_important_change], noun);
  });
  let branch = $derived(git?.branch ?? (git?.commit ? `detached at ${git.commit}` : name));
</script>

<ChoiceMenu label="Git" title="Git: {summary}">
  {#snippet children({ close })}
    {#if dirty}
      <li class="menu-title whitespace-normal font-normal text-base-content/70">
        <p class="git-summary">{summary}</p>
        <dl class="git-details">
          <dt>Branch</dt>
          <dd>{branch}</dd>
          {#if git.upstream}
            <dt>Upstream</dt>
            <dd>{git.upstream}</dd>
            <dt>Commits</dt>
            <dd>{git.ahead} ahead, {git.behind} behind</dd>
          {/if}
          {#each CHANGES as [key, , label] (key)}
            <dt class={{ [`git-${key}`]: git[key] > 0 }}>{label}</dt>
            <dd>{git[key]}</dd>
          {/each}
        </dl>
        <p class="git-hint">Commit or remove the changes before turning it into a branch or merging it into {mainBranch}.</p>
      </li>
    {:else}
      <MenuChoice
        label="Turn into branch"
        explanation="Removes the worktree and closes the session but keeps the branch {branch}, to check out, merge or delete later from the main checkout."
        onclick={() => {
          close();
          onClose(name);
        }}
      />
      <MenuChoice
        label="Merge into {mainBranch}"
        explanation="Fast-forwards {mainBranch} with {branch}, then removes the worktree, deletes the branch and closes the session. The work lives on in {mainBranch}."
        onclick={() => {
          close();
          onMerge(name);
        }}
      />
    {/if}
  {/snippet}
</ChoiceMenu>

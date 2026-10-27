<!-- The Complete menu on a session or worktree card: the two ways to
     finish, as bin/worktree has them. Both need a clean worktree. -->
<script>
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";

  let { name, git, mainBranch, onMerge, onClose } = $props();

  let dirty = $derived(git?.dirty ?? false);
</script>

<ChoiceMenu label="Complete" title="Finish the session: merge it or turn it into a branch">
  {#snippet children({ close })}
    <MenuChoice
      label="Merge into {mainBranch}"
      explanation="Fast-forwards {mainBranch} with the branch, then removes the worktree, deletes the branch and closes the session. The work lives on in {mainBranch}."
      disabled={dirty}
      onclick={() => {
        close();
        onMerge(name);
      }}
    />
    <MenuChoice
      label="Turn into branch"
      explanation="Removes the worktree and closes the session but keeps the branch, to check out, merge or delete later from the main checkout."
      disabled={dirty}
      onclick={() => {
        close();
        onClose(name);
      }}
    />
    {#if dirty}
      <li class="menu-title whitespace-normal">The worktree has uncommitted changes. Commit or remove them first.</li>
    {/if}
  {/snippet}
</ChoiceMenu>

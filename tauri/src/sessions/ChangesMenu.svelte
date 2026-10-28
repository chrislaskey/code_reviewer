<!-- A menu on a session or worktree card that opens the changes in its
     directory in another feature: the uncommitted changes, or everything
     the branch committed since the main branch. Each choice is a plain link
     to the page `urlFor` names, such as the review or the trace show page. -->
<script>
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";

  /**
   * @type {{
   *   label: string,
   *   title: string,
   *   directory: string,
   *   git: any,
   *   mainBranch: string,
   *   urlFor: (request: object) => string,
   * }}
   */
  let { label, title, directory, git, mainBranch, urlFor } = $props();

  let uncommittedChangesUrl = $derived(urlFor({ repo_path: directory }));
  let committedChangesUrl = $derived(
    urlFor({
      repo_path: directory,
      base_ref: mainBranch,
      head_ref: git?.branch ?? "HEAD",
      compare_from_merge_base: true,
    }),
  );
</script>

<ChoiceMenu {label} {title}>
  {#snippet children({ close })}
    <MenuChoice
      label="Uncommitted changes"
      explanation="The uncommitted changes in the worktree, compared with its last commit."
      href={uncommittedChangesUrl}
      onclick={close}
    />
    <MenuChoice
      label="Committed changes"
      explanation="Everything the branch committed since it branched off {mainBranch}, like a pull request."
      href={committedChangesUrl}
      onclick={close}
    />
  {/snippet}
</ChoiceMenu>

<!-- The Review menu on a session or worktree card: links into Reviews for
     the changes in its directory, each a plain link to the show page. -->
<script>
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";
  import { reviewShowUrl } from "../lib/routes.js";

  let { directory, git, mainBranch } = $props();

  let recentChangesUrl = $derived(reviewShowUrl({ repo_path: directory }));
  let allChangesUrl = $derived(
    reviewShowUrl({
      repo_path: directory,
      base_ref: mainBranch,
      head_ref: git?.branch ?? "HEAD",
      compare_from_merge_base: true,
    }),
  );
</script>

<ChoiceMenu label="Review" title="Open the changes in Reviews">
  {#snippet children({ close })}
    <MenuChoice
      label="Recent changes"
      explanation="The uncommitted changes in the worktree, compared with its last commit."
      href={recentChangesUrl}
      onclick={close}
    />
    <MenuChoice
      label="All changes"
      explanation="Everything the branch changed since it branched off {mainBranch}, like a pull request."
      href={allChangesUrl}
      onclick={close}
    />
  {/snippet}
</ChoiceMenu>

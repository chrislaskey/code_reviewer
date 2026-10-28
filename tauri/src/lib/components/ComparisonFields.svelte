<!-- The fields that say what to compare: the repository path, the mode and,
     by mode, the commit or the two branches. Reads the repository's branches
     and commits once the path stops changing and keeps the form's choices
     valid (see lib/comparison_form.js). -->
<script module>
  /** The classes of an input or select on the new review and new trace pages. */
  export const FIELD_CLASS =
    "rounded-lg border border-base-300 bg-base-100 px-3 py-2 font-mono text-sm text-base-content placeholder:text-base-content/30";
</script>

<script>
  import { listBranchesAndRecentCommits } from "../backend.js";
  import { COMPARE_MODE_OPTIONS, withValidChoices } from "../comparison_form.js";
  import SegmentedControl from "./SegmentedControl.svelte";

  /** @type {{ form: any, onRefsError?: (message: string | null) => void }} */
  let { form = $bindable(), onRefsError } = $props();

  let refs = $state(null);

  let branchOptions = $derived((refs?.branches ?? []).map((branch) => ({ value: branch, label: branch })));
  let commitOptions = $derived(
    (refs?.recent_commits ?? []).map((commit) => ({
      value: commit.sha,
      label: `${commit.short_sha}  ${commit.subject}  (${commit.committed_on})`,
    })),
  );

  // Until the repository has been read, a select shows the remembered choice.
  function withRemembered(value, options) {
    if (value === "" || options.some((option) => option.value === value)) return options;
    const label = /^[0-9a-f]{40}$/.test(value) ? value.slice(0, 8) : value;
    return [{ value, label }, ...options];
  }

  // Read the branches and commits once the path stops changing.
  $effect(() => {
    const repoPath = form.repoPath.trim();
    if (repoPath === "") {
      refs = null;
      onRefsError?.(null);
      return;
    }
    let outdated = false;
    const timer = window.setTimeout(async () => {
      try {
        const response = await listBranchesAndRecentCommits({ repo_path: repoPath });
        if (outdated) return;
        refs = response;
        onRefsError?.(null);
        form = withValidChoices(form, response);
      } catch (error) {
        if (outdated) return;
        refs = null;
        onRefsError?.(String(error));
      }
    }, 400);
    return () => {
      outdated = true;
      window.clearTimeout(timer);
    };
  });
</script>

<div class="flex flex-wrap items-end gap-4">
  <label class="flex min-w-72 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
    Repository path
    <input bind:value={form.repoPath} placeholder="/path/to/project" spellcheck="false" class={FIELD_CLASS} />
  </label>
  <SegmentedControl size="medium" value={form.mode} options={COMPARE_MODE_OPTIONS} onchange={(mode) => (form.mode = mode)} />
</div>

{#if form.mode === "commit"}
  <label class="flex flex-col gap-1.5 text-sm text-base-content/60">
    Commit
    <select bind:value={form.commitSha} class={FIELD_CLASS}>
      {#each withRemembered(form.commitSha, commitOptions) as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {:else}
        <option value="">None found</option>
      {/each}
    </select>
  </label>
{/if}
{#if form.mode === "branch"}
  <div class="flex flex-wrap items-end gap-4">
    <label class="flex min-w-48 flex-col gap-1.5 text-sm text-base-content/60">
      This branch
      <select bind:value={form.headBranch} class={FIELD_CLASS}>
        {#each withRemembered(form.headBranch, branchOptions) as option (option.value)}
          <option value={option.value}>{option.label}</option>
        {:else}
          <option value="">None found</option>
        {/each}
      </select>
    </label>
    <label class="flex min-w-48 flex-col gap-1.5 text-sm text-base-content/60">
      Against
      <select bind:value={form.baseBranch} class={FIELD_CLASS}>
        {#each withRemembered(form.baseBranch, branchOptions) as option (option.value)}
          <option value={option.value}>{option.label}</option>
        {:else}
          <option value="">None found</option>
        {/each}
      </select>
    </label>
  </div>
{/if}

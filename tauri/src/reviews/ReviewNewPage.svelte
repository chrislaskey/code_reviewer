<!-- The new review page: pick the repository, what to compare, and which
     modules to highlight or ignore. Submitting opens the show page. -->
<script>
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import { listBranchesAndRecentCommits } from "../lib/backend.js";
  import SegmentedControl from "./SegmentedControl.svelte";
  import {
    COMPARE_MODE_OPTIONS,
    buildRequest,
    describeComparison,
    loadForm,
    saveForm,
    showPageUrl,
    withValidChoices,
  } from "./review_form.js";

  const FIELD_CLASS =
    "rounded-lg border border-base-300 bg-base-100 px-2.5 py-1.5 font-mono text-xs text-base-content placeholder:text-base-content/30";

  let form = $state(loadForm());
  let refs = $state(null);
  let refsError = $state(null);

  let request = $derived(buildRequest(form));
  let branchOptions = $derived((refs?.branches ?? []).map((branch) => ({ value: branch, label: branch })));
  let commitOptions = $derived(
    (refs?.recent_commits ?? []).map((commit) => ({
      value: commit.sha,
      label: `${commit.short_sha}  ${commit.subject}  (${commit.committed_on})`,
    })),
  );
  let hasRules = $derived(form.includeRules.trim() !== "" || form.ignoreRules.trim() !== "");
  let hasGroups = $derived(
    form.groupConfigPath.trim() !== "" || form.groupIncludes.trim() !== "" || form.groupExcludes.trim() !== "",
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
      refsError = null;
      return;
    }
    let outdated = false;
    const timer = window.setTimeout(async () => {
      try {
        const response = await listBranchesAndRecentCommits({ repo_path: repoPath });
        if (outdated) return;
        refs = response;
        refsError = null;
        form = withValidChoices(form, response);
      } catch (error) {
        if (outdated) return;
        refs = null;
        refsError = String(error);
      }
    }, 400);
    return () => {
      outdated = true;
      window.clearTimeout(timer);
    };
  });

  function submit(event) {
    event.preventDefault();
    if (request === null) return;
    saveForm(form);
    window.location.href = showPageUrl(request);
  }
</script>

<AppNav current="reviews" />

<main class="mx-auto max-w-3xl px-6 py-6">
  <Header>
    New review
    {#snippet subtitle()}
      A git repository with Elixir code, and what to compare.
    {/snippet}
  </Header>

  <form onsubmit={submit} class="space-y-4 rounded-xl border border-base-300 bg-base-100 px-4 py-4">
    <div class="flex flex-wrap items-end gap-3">
      <label class="flex min-w-72 flex-1 flex-col gap-1 text-[11px] text-base-content/60">
        Repository path
        <input bind:value={form.repoPath} placeholder="/path/to/elixir/project" spellcheck="false" class={FIELD_CLASS} />
      </label>
      <SegmentedControl value={form.mode} options={COMPARE_MODE_OPTIONS} onchange={(mode) => (form.mode = mode)} />
    </div>

    {#if form.mode === "commit"}
      <label class="flex flex-col gap-1 text-[11px] text-base-content/60">
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
      <div class="flex flex-wrap items-end gap-3">
        <label class="flex min-w-48 flex-col gap-1 text-[11px] text-base-content/60">
          Review this branch
          <select bind:value={form.headBranch} class={FIELD_CLASS}>
            {#each withRemembered(form.headBranch, branchOptions) as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {:else}
              <option value="">None found</option>
            {/each}
          </select>
        </label>
        <label class="flex min-w-48 flex-col gap-1 text-[11px] text-base-content/60">
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

    <details open={hasRules}>
      <summary class="cursor-pointer text-[11px] text-base-content/60">Highlight or ignore modules</summary>
      <div class="mt-2 space-y-2">
        <div class="flex flex-wrap gap-3">
          <label class="flex min-w-56 flex-1 flex-col gap-1 text-[11px] text-base-content/60">
            Also highlight (one per line)
            <textarea bind:value={form.includeRules} placeholder={"Ecto.Repo\nShopWeb"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
          <label class="flex min-w-56 flex-1 flex-col gap-1 text-[11px] text-base-content/60">
            Ignore (one per line)
            <textarea bind:value={form.ignoreRules} placeholder={"Enum.reduce\nShopTest*"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
        </div>
        <p class="text-[11px] text-base-content/50">
          Calls to modules the project defines are always highlighted. A line is a module (<code>Shop</code> covers
          <code>Shop.Orders</code> but not <code>ShopWeb</code>), a function (<code>Enum.reduce</code> or
          <code>Enum.reduce/3</code>), or a prefix ending in <code>*</code> (<code>Shop*</code> covers <code>ShopWeb</code>
          too). Ignored callers and calls disappear; ignoring beats highlighting.
        </p>
      </div>
    </details>

    <details open={hasGroups}>
      <summary class="cursor-pointer text-[11px] text-base-content/60">Groups</summary>
      <div class="mt-2 space-y-2">
        <label class="flex max-w-xl flex-col gap-1 text-[11px] text-base-content/60">
          Group config file (optional)
          <input
            bind:value={form.groupConfigPath}
            placeholder=".code_reviewer.toml in the repository is used when this is empty"
            spellcheck="false"
            class={FIELD_CLASS}
          />
        </label>
        <div class="flex flex-wrap gap-3">
          <label class="flex min-w-56 flex-1 flex-col gap-1 text-[11px] text-base-content/60">
            Add to a group (one per line)
            <textarea bind:value={form.groupIncludes} placeholder={"data=lib/shop/repo/**\ntests=test/**"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
          <label class="flex min-w-56 flex-1 flex-col gap-1 text-[11px] text-base-content/60">
            Leave out of a group (one per line)
            <textarea bind:value={form.groupExcludes} placeholder={"data=**/*_test.exs"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
        </div>
        <p class="text-[11px] text-base-content/50">
          A line is <code>group=pattern</code>, where the group is a name (<code>data</code>) and the pattern is a glob
          or <code>regex:</code> expression matched against the path from the repository root. These add to
          <code>.code_reviewer.toml</code>. With no groups defined, every file is in the default group.
        </p>
      </div>
    </details>

    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" type="submit" disabled={request === null}>Review changes</Button>
      <p class="text-[11px] text-base-content/50">{describeComparison(form)}</p>
      {#if refsError && form.mode !== "working_tree"}
        <p class="text-[11px] text-rose-600 dark:text-rose-400">Could not read the branches: {refsError}</p>
      {/if}
    </div>
  </form>
</main>

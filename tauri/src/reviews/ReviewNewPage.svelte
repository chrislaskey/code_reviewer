<!-- The new review page: pick the repository, what to compare, and which
     modules to highlight or ignore. Submitting opens the show page. Opened
     with a request in its URL, as from Edit review, the form starts from it;
     otherwise from the last one submitted. -->
<script>
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import ComparisonFields, { FIELD_CLASS } from "../lib/components/ComparisonFields.svelte";
  import Header from "../lib/components/Header.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import { describeComparison } from "../lib/comparison_form.js";
  import { reviewRequestFromSearch, reviewShowUrl } from "../lib/routes.js";
  import { buildRequest, formFromRequest, loadForm, saveForm } from "./review_form.js";

  const linkedRequest = reviewRequestFromSearch(window.location.search);

  let form = $state(linkedRequest ? formFromRequest(linkedRequest) : loadForm());
  let refsError = $state(null);

  let request = $derived(buildRequest(form));
  let hasRules = $derived(form.includeRules.trim() !== "" || form.ignoreRules.trim() !== "");
  let hasGroups = $derived(
    form.groupConfigPath.trim() !== "" || form.groupIncludes.trim() !== "" || form.groupExcludes.trim() !== "",
  );

  function submit(event) {
    event.preventDefault();
    if (request === null) return;
    saveForm(form);
    window.location.href = reviewShowUrl(request);
  }
</script>

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="reviews" />

<main class="page with-waves">
  <Header>
    New review
    {#snippet subtitle()}
      A git repository with Elixir or Rust code, and what to compare.
    {/snippet}
  </Header>

  <form onsubmit={submit} class="space-y-5 rounded-xl border border-base-300 bg-base-100 p-6">
    <ComparisonFields bind:form onRefsError={(message) => (refsError = message)} />

    <details open={hasRules}>
      <summary class="cursor-pointer text-sm text-base-content/60">Highlight or ignore modules</summary>
      <div class="mt-3 space-y-3">
        <div class="flex flex-wrap gap-4">
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            Also highlight (one per line)
            <textarea bind:value={form.includeRules} placeholder={"Ecto.Repo\nShopWeb"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            Ignore (one per line)
            <textarea bind:value={form.ignoreRules} placeholder={"Enum.reduce\nShopTest*"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
        </div>
        <p class="text-sm text-base-content/50">
          Calls to modules the project defines are always highlighted. A line is a module (<code>Shop</code> covers
          <code>Shop.Orders</code> but not <code>ShopWeb</code>), a function (<code>Enum.reduce</code> or
          <code>Enum.reduce/3</code>), or a prefix ending in <code>*</code> (<code>Shop*</code> covers <code>ShopWeb</code>
          too). Ignored callers and calls disappear; ignoring beats highlighting.
        </p>
      </div>
    </details>

    <details open={hasGroups}>
      <summary class="cursor-pointer text-sm text-base-content/60">Groups</summary>
      <div class="mt-3 space-y-3">
        <label class="flex max-w-xl flex-col gap-1.5 text-sm text-base-content/60">
          Group config file (optional)
          <input
            bind:value={form.groupConfigPath}
            placeholder=".code_reviewer.toml in the repository is used when this is empty"
            spellcheck="false"
            class={FIELD_CLASS}
          />
        </label>
        <div class="flex flex-wrap gap-4">
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            Add to a group (one per line)
            <textarea bind:value={form.groupIncludes} placeholder={"data=lib/shop/repo/**\ntests=test/**"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            Leave out of a group (one per line)
            <textarea bind:value={form.groupExcludes} placeholder={"data=**/*_test.exs"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
        </div>
        <p class="text-sm text-base-content/50">
          A line is <code>group=pattern</code>, where the group is a name (<code>data</code>) and the pattern is a glob
          or <code>regex:</code> expression matched against the path from the repository root. These add to
          <code>.code_reviewer.toml</code>. With no groups defined, every file is in the default group.
        </p>
      </div>
    </details>

    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" type="submit" disabled={request === null}>Review changes</Button>
      <p class="text-sm text-base-content/50">{describeComparison(form)}</p>
      {#if refsError && form.mode !== "working_tree"}
        <p class="text-sm text-error">Could not read the branches: {refsError}</p>
      {/if}
    </div>
  </form>
</main>

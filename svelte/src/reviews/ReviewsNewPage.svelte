<!-- The new review page: pick the repository, what to compare, and which
     modules to highlight or ignore. Submitting opens the show page. Opened
     with a request in its URL, as from Edit review, the form starts from it;
     otherwise from the last one submitted. -->
<script>
  import { jt, t } from "ttag";
  import { Page, TranslatedParts } from "../lib/components/index.js";
  import Button from "../lib/components/Button.svelte";
  import ComparisonFields, { FIELD_CLASS } from "../lib/code_reviewer/components/ComparisonFields.svelte";
  import Header from "../lib/code_reviewer/components/Header.svelte";
  import { describeComparison } from "../lib/code_reviewer/comparison_form.js";
  import { navigateToUrl } from "../lib/router.js";
  import { reviewRequestFromSearch, reviewsShowUrl } from "../lib/routes.js";
  import { buildRequest, formFromRequest, loadForm, saveForm } from "./review_form.js";
  import StillWaves from "../lib/code_reviewer/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/code_reviewer/components/TopBackgroundPattern.svelte";

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
    navigateToUrl(reviewsShowUrl(request));
  }
</script>

<!-- untranslated: code -->
{#snippet shop()}<code>Shop</code>{/snippet}
<!-- untranslated: code -->
{#snippet shopOrders()}<code>Shop.Orders</code>{/snippet}
<!-- untranslated: code -->
{#snippet shopWeb()}<code>ShopWeb</code>{/snippet}
<!-- untranslated: code -->
{#snippet enumReduce()}<code>Enum.reduce</code>{/snippet}
<!-- untranslated: code -->
{#snippet enumReduceArity()}<code>Enum.reduce/3</code>{/snippet}
<!-- untranslated: code -->
{#snippet star()}<code>*</code>{/snippet}
<!-- untranslated: code -->
{#snippet shopStar()}<code>Shop*</code>{/snippet}
<!-- untranslated: code -->
{#snippet groupPattern()}<code>group=pattern</code>{/snippet}
<!-- untranslated: code -->
{#snippet data()}<code>data</code>{/snippet}
<!-- untranslated: code -->
{#snippet regex()}<code>regex:</code>{/snippet}
<!-- untranslated: code -->
{#snippet groupConfigFile()}<code>.code_reviewer.toml</code>{/snippet}

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<Page wide class="with-waves">
  <Header>
    {t`New review`}
    {#snippet subtitle()}
      {t`A git repository with Elixir or Rust code, and what to compare.`}
    {/snippet}
  </Header>

  <form onsubmit={submit} class="raised-panel space-y-5 p-6">
    <ComparisonFields bind:form onRefsError={(message) => (refsError = message)} />

    <details open={hasRules}>
      <summary class="cursor-pointer text-sm text-base-content/60">{t`Highlight or ignore modules`}</summary>
      <div class="mt-3 space-y-3">
        <div class="flex flex-wrap gap-4">
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            {t`Also highlight (one per line)`}
            <!-- untranslated: code -->
            <textarea bind:value={form.includeRules} placeholder={"Ecto.Repo\nShopWeb"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            {t`Ignore (one per line)`}
            <!-- untranslated: code -->
            <textarea bind:value={form.ignoreRules} placeholder={"Enum.reduce\nShopTest*"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
        </div>
        <p class="text-sm text-base-content/50">
          <TranslatedParts
            parts={jt`Calls to modules the project defines are always highlighted. A line is a module (${shop} covers ${shopOrders} but not ${shopWeb}), a function (${enumReduce} or ${enumReduceArity}), or a prefix ending in ${star} (${shopStar} covers ${shopWeb} too). Ignored callers and calls disappear; ignoring beats highlighting.`}
          />
        </p>
      </div>
    </details>

    <details open={hasGroups}>
      <summary class="cursor-pointer text-sm text-base-content/60">{t`Groups`}</summary>
      <div class="mt-3 space-y-3">
        <label class="flex max-w-xl flex-col gap-1.5 text-sm text-base-content/60">
          {t`Group config file (optional)`}
          <input
            bind:value={form.groupConfigPath}
            placeholder={t`.code_reviewer.toml in the repository is used when this is empty`}
            spellcheck="false"
            class={FIELD_CLASS}
          />
        </label>
        <div class="flex flex-wrap gap-4">
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            {t`Add to a group (one per line)`}
            <!-- untranslated: code -->
            <textarea bind:value={form.groupIncludes} placeholder={"data=lib/shop/repo/**\ntests=test/**"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
          <label class="flex min-w-56 flex-1 flex-col gap-1.5 text-sm text-base-content/60">
            {t`Leave out of a group (one per line)`}
            <!-- untranslated: code -->
            <textarea bind:value={form.groupExcludes} placeholder={"data=**/*_test.exs"} rows="3" spellcheck="false" class={FIELD_CLASS}></textarea>
          </label>
        </div>
        <p class="text-sm text-base-content/50">
          <TranslatedParts
            parts={jt`A line is ${groupPattern}, where the group is a name (${data}) and the pattern is a glob or ${regex} expression matched against the path from the repository root. These add to ${groupConfigFile}. With no groups defined, every file is in the default group.`}
          />
        </p>
      </div>
    </details>

    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" type="submit" disabled={request === null}>{t`Review changes`}</Button>
      <p class="text-sm text-base-content/50">{describeComparison(form)}</p>
      {#if refsError && form.mode !== "working_tree"}
        <p class="text-sm text-error">{t`Could not read the branches: ${refsError}`}</p>
      {/if}
    </div>
  </form>
</Page>

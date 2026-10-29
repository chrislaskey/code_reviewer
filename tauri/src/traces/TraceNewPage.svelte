<!-- The new trace page: pick the repository and what to compare. Submitting
     opens the show page, which lists the tests to pick one from. Opened with
     a request in its URL, as from Edit trace, the form starts from it;
     otherwise from the last one submitted. -->
<script>
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import ComparisonFields from "../lib/components/ComparisonFields.svelte";
  import Header from "../lib/components/Header.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import {
    EMPTY_COMPARISON_FORM,
    buildComparison,
    comparisonFormFromRequest,
    describeComparison,
    loadForm,
    saveForm,
  } from "../lib/comparison_form.js";
  import { traceRequestFromSearch, traceShowUrl } from "../lib/routes.js";

  const STORAGE_KEY = "code_reviewer.trace_form.v1";

  const linkedRequest = traceRequestFromSearch(window.location.search);

  let form = $state(linkedRequest ? comparisonFormFromRequest(linkedRequest) : loadForm(STORAGE_KEY, EMPTY_COMPARISON_FORM));
  let refsError = $state(null);

  let request = $derived(buildComparison(form));

  function submit(event) {
    event.preventDefault();
    if (request === null) return;
    saveForm(STORAGE_KEY, form);
    window.location.href = traceShowUrl(request);
  }
</script>

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="traces" />

<main class="page with-waves">
  <Header>
    New trace
    {#snippet subtitle()}
      A git repository with Elixir or Rust code, and what to compare. The next page lists its tests to pick one from.
    {/snippet}
  </Header>

  <form onsubmit={submit} class="space-y-5 rounded-xl border border-base-300 bg-base-100 p-6">
    <ComparisonFields bind:form onRefsError={(message) => (refsError = message)} />

    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" type="submit" disabled={request === null}>List tests</Button>
      <p class="text-sm text-base-content/50">{describeComparison(form)}</p>
      {#if refsError && form.mode !== "working_tree"}
        <p class="text-sm text-error">Could not read the branches: {refsError}</p>
      {/if}
    </div>
  </form>
</main>

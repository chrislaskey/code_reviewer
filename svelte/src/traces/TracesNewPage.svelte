<!-- The new trace page: pick the repository and what to compare. Submitting
     opens the show page, which lists the tests to pick one from. Opened with
     a request in its URL, as from Edit trace, the form starts from it;
     otherwise from the last one submitted. -->
<script>
  import { Page } from "../lib/components/index.js";
  import Button from "../lib/components/Button.svelte";
  import ComparisonFields from "../lib/code_reviewer/components/ComparisonFields.svelte";
  import Header from "../lib/code_reviewer/components/Header.svelte";
  import {
    EMPTY_COMPARISON_FORM,
    buildComparison,
    comparisonFormFromRequest,
    describeComparison,
    loadForm,
    saveForm,
  } from "../lib/code_reviewer/comparison_form.js";
  import { navigateToUrl } from "../lib/router.js";
  import { traceRequestFromSearch, tracesShowUrl } from "../lib/routes.js";

  const STORAGE_KEY = "code_reviewer.trace_form.v1";

  const linkedRequest = traceRequestFromSearch(window.location.search);

  let form = $state(linkedRequest ? comparisonFormFromRequest(linkedRequest) : loadForm(STORAGE_KEY, EMPTY_COMPARISON_FORM));
  let refsError = $state(null);

  let request = $derived(buildComparison(form));

  function submit(event) {
    event.preventDefault();
    if (request === null) return;
    saveForm(STORAGE_KEY, form);
    navigateToUrl(tracesShowUrl(request));
  }
</script>




<Page wide>
  <Header>
    New trace
    {#snippet subtitle()}
      A git repository with Elixir or Rust code, and what to compare. The next page lists its tests to pick one from.
    {/snippet}
  </Header>

  <form onsubmit={submit} class="raised-panel space-y-5 p-6">
    <ComparisonFields bind:form onRefsError={(message) => (refsError = message)} />

    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" type="submit" disabled={request === null}>List tests</Button>
      <p class="text-sm text-base-content/50">{describeComparison(form)}</p>
      {#if refsError && form.mode !== "working_tree"}
        <p class="text-sm text-error">Could not read the branches: {refsError}</p>
      {/if}
    </div>
  </form>
</Page>

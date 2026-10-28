<!-- The trace page: lists the tests in the head of the comparison in the
     URL, and walks the calls below the picked test, top to bottom like a
     stack trace, with each function marked against the base. The picked
     test is kept in the URL, so the page reloads on the same trace. -->
<script>
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import { getTestCallStack, listTests } from "../lib/backend.js";
  import { describeRequest } from "../lib/comparison_form.js";
  import { traceNewUrl, traceRequestFromSearch, traceShowUrl } from "../lib/routes.js";
  import { testLabel } from "./call_stack.js";
  import CallStackView from "./CallStackView.svelte";
  import TestPicker from "./TestPicker.svelte";

  const request = traceRequestFromSearch(window.location.search);
  /** The test named in the URL, if any, and the comparison without it. */
  const linkedTest = request?.test ?? null;
  const comparison = request ? withoutTest(request) : null;

  /** @param {any} request */
  function withoutTest({ test: _test, ...comparison }) {
    return comparison;
  }

  /** @typedef {{ state: "idle" } | { state: "loading" } | { state: "failed", message: string } | { state: "ready", value: any }} Loaded */
  let tests = $state(/** @type {Loaded} */ ({ state: request ? "loading" : "idle" }));
  let selected = $state(null);
  let stack = $state(/** @type {Loaded} */ ({ state: "idle" }));

  function shortCommit(commit) {
    return commit.slice(0, 8);
  }

  async function selectTest(test) {
    selected = test;
    window.history.replaceState(null, "", traceShowUrl({ ...comparison, test }));
    stack = { state: "loading" };
    try {
      stack = { state: "ready", value: await getTestCallStack({ ...comparison, test }) };
    } catch (error) {
      stack = { state: "failed", message: String(error) };
    }
  }

  onMount(async () => {
    if (request === null) return;
    try {
      tests = { state: "ready", value: await listTests(comparison) };
    } catch (error) {
      tests = { state: "failed", message: String(error) };
      return;
    }
    if (linkedTest) await selectTest(linkedTest);
  });
</script>

<div class="full-page">
  <AppNav current="traces" />

  <div class="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b border-base-300 bg-base-100 px-4 py-2 font-mono text-[11px] text-base-content/70">
    {#if request}
      <span class="text-base-content">{request.repo_path}</span>
      <span>{describeRequest(request)}</span>
      {#if tests.state === "ready"}
        <span>
          {tests.value.tests.length} tests ·
          {shortCommit(tests.value.base_commit)} → {tests.value.head_commit ? shortCommit(tests.value.head_commit) : "working tree"}
        </span>
      {/if}
      {#if selected}
        <span class="text-base-content">{selected.module} · {testLabel(selected)}</span>
      {/if}
    {/if}
    <a href={traceNewUrl(request)} class="ml-auto text-primary">Edit trace</a>
  </div>

  {#if request === null}
    <p class="px-6 py-10 text-sm text-base-content/50">
      This page needs a comparison to list the tests of. <a href={traceNewUrl()}>Start a new trace.</a>
    </p>
  {:else if tests.state === "loading"}
    <p class="px-6 py-10 text-sm text-base-content/50">Listing the tests…</p>
  {:else if tests.state === "failed"}
    <p class="border-b border-base-300 bg-error/10 px-4 py-3 font-mono text-xs text-error">{tests.message}</p>
  {:else if tests.state === "ready"}
    <div class="full-page-body">
      <aside class="flex w-80 shrink-0 flex-col border-r border-base-300 bg-base-200">
        <TestPicker tests={tests.value.tests} {selected} onSelect={(test) => selectTest(test.function)} />
      </aside>
      <div class="full-page-content flex flex-col">
        {#if stack.state === "idle"}
          <p class="px-6 py-10 text-sm text-base-content/50">Pick a test to walk its calls.</p>
        {:else if stack.state === "loading"}
          <p class="px-6 py-10 text-sm text-base-content/50">Walking the calls…</p>
        {:else if stack.state === "failed"}
          <p class="border-b border-base-300 bg-error/10 px-4 py-3 font-mono text-xs text-error">{stack.message}</p>
        {:else}
          <CallStackView stack={stack.value} />
        {/if}
      </div>
    </div>
  {/if}
</div>

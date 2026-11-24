<!-- The trace page: lists the tests in the head of the comparison in the
     URL, and walks the calls below the picked test, top to bottom like a
     stack trace, with each function marked against the base. The picked
     test is kept in the URL, so the page reloads on the same trace.
     Small windows have one column, switched by the Tests and Calls tabs:
     the tests, full width, or only the calls. Picking a test shows its calls. -->
<script>
  import { onMount } from "svelte";
  import { jt, msgid, ngettext, t } from "ttag";
  import ChoiceMenu from "../lib/code_reviewer/components/ChoiceMenu.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import TranslatedParts from "../lib/components/TranslatedParts.svelte";
  import InfoPopover from "../lib/code_reviewer/components/InfoPopover.svelte";
  import MenuChoice from "../lib/code_reviewer/components/MenuChoice.svelte";
  import SegmentedControl from "../lib/code_reviewer/components/SegmentedControl.svelte";
  import Toolbar from "../lib/code_reviewer/components/Toolbar.svelte";
  import { tracesCallStacksGet, tracesTestsList } from "../lib/core/index.js";
  import { describeRequest } from "../lib/code_reviewer/comparison_form.js";
  import { tracesNewUrl, traceRequestFromSearch, tracesShowUrl } from "../lib/routes.js";
  import { frameCounts, stopLabels, systemCallChoices, testLabel, unchangedChoices } from "./call_stack.js";
  import CallStackView from "./CallStackView.svelte";
  import TestPicker from "./TestPicker.svelte";
  import TopBackgroundPattern from "../lib/code_reviewer/components/TopBackgroundPattern.svelte";

  const PHONE_TAB_OPTIONS = [
    { value: "tests", label: t`Tests` },
    { value: "calls", label: t`Calls` },
  ];
  const SYSTEM_CALL_CHOICES = systemCallChoices();
  const UNCHANGED_CHOICES = unchangedChoices();
  const STOP_LABELS = stopLabels();

  const ICON_BUTTON = "btn btn-ghost btn-sm btn-square";

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
  /** The one column small windows show. */
  let phoneTab = $state(linkedTest ? "calls" : "tests");
  let systemCalls = $state(/** @type {"shown" | "hidden"} */ ("hidden"));
  let unchanged = $state(/** @type {"shown" | "hidden"} */ ("shown"));
  let callStackView = $state();

  let counts = $derived(stack.state === "ready" ? frameCounts(stack.value.frames) : null);
  let truncated = $derived(stack.state === "ready" && stack.value.truncated);
  let systemCallsLabel = $derived(SYSTEM_CALL_CHOICES.find((choice) => choice.value === systemCalls)?.label);
  let unchangedLabel = $derived(UNCHANGED_CHOICES.find((choice) => choice.value === unchanged)?.label);

  function shortCommit(commit) {
    return commit.slice(0, 8);
  }

  function functionsCount(count) {
    return ngettext(msgid`${count} function`, `${count} functions`, count);
  }

  async function selectTest(test) {
    selected = test;
    // The page's own address, not a navigation: the router's history entry stays.
    window.history.replaceState(window.history.state, "", tracesShowUrl({ ...comparison, test }));
    stack = { state: "loading" };
    try {
      stack = { state: "ready", value: await tracesCallStacksGet({ ...comparison, test }) };
    } catch (error) {
      stack = { state: "failed", message: String(error) };
    }
  }

  /** From the picker: shows the test's calls, from the top on small windows, where the page scrolls. */
  function pickTest(test) {
    phoneTab = "calls";
    window.scrollTo({ top: 0 });
    selectTest(test.function);
  }

  onMount(async () => {
    if (request === null) return;
    try {
      tests = { state: "ready", value: await tracesTestsList(comparison) };
    } catch (error) {
      tests = { state: "failed", message: String(error) };
      return;
    }
    if (linkedTest) await selectTest(linkedTest);
  });
</script>

{#snippet iconButton(icon, title, onclick, disabled = false)}
  <button type="button" class={ICON_BUTTON} {title} aria-label={title} {disabled} {onclick}>
    <Icon name={icon} class="size-4" />
  </button>
{/snippet}

{#snippet choiceMenu(title, prefix, label, choices, onchoose)}
  <ChoiceMenu {title} class="btn btn-ghost btn-sm" align="start">
    {#snippet buttonContent()}
      {#if prefix}<span class="max-sm:hidden">{prefix}</span>{/if}
      {label}
    {/snippet}
    {#snippet children({ close })}
      {#each choices as choice (choice.value)}
        <MenuChoice
          label={choice.label}
          explanation={choice.explanation}
          onclick={() => {
            onchoose(choice.value === "shown" ? "shown" : "hidden");
            close();
          }}
        />
      {/each}
    {/snippet}
  </ChoiceMenu>
{/snippet}

<TopBackgroundPattern />

<div class="full-page max-sm:h-auto">
  <div class="full-page-title">
    <h1>{t`Trace`}</h1>
    {#if request}
      <span class="comparison font-mono text-[11px] text-base-content/70">
        <span class="text-base-content">{request.repo_path}</span>
        <span>{describeRequest(request)}</span>
      </span>
    {/if}
    <a href={tracesNewUrl(request)} class="btn btn-ghost btn-sm ml-auto">
      <Icon name="hero-pencil-square-micro" class="size-4" />
      {t`Edit`}
    </a>
  </div>

  {#if request === null}
    {#snippet newTraceLink()}<a href={tracesNewUrl()}>{t`Start a new trace.`}</a>{/snippet}
    <p class="px-6 py-10 text-sm text-base-content/50">
      <TranslatedParts parts={jt`This page needs a comparison to list the tests of. ${newTraceLink}`} />
    </p>
  {:else if tests.state === "loading"}
    <p class="px-6 py-10 text-sm text-base-content/50">{t`Listing the tests…`}</p>
  {:else if tests.state === "failed"}
    <p class="border-b border-base-300 bg-error/10 px-4 py-3 font-mono text-xs text-error">{tests.message}</p>
  {:else if tests.state === "ready"}
    <!-- On small windows the page scrolls as a whole, and the toolbar stays at the top. -->
    <Toolbar class="full-page-toolbar">
      <div class="sm:hidden">
        <SegmentedControl value={phoneTab} options={PHONE_TAB_OPTIONS} onchange={(value) => (phoneTab = value)} />
      </div>
      {#if counts}
        <div class={["flex flex-wrap items-center gap-1", phoneTab === "tests" && "max-sm:hidden"]}>
          {@render choiceMenu(t`Which calls the stack shows`, t`Show`, systemCallsLabel, SYSTEM_CALL_CHOICES, (value) => (systemCalls = value))}
          {@render choiceMenu(t`Which functions the stack shows`, null, unchangedLabel, UNCHANGED_CHOICES, (value) => (unchanged = value))}
          <div class="flex items-center">
            {@render iconButton("lucide-chevrons-up-down", t`Open the code of every function`, () => callStackView?.setAllCode(true))}
            {@render iconButton("lucide-chevrons-down-up", t`Close the code of every function`, () => callStackView?.setAllCode(false))}
          </div>
          <div class="flex items-center">
            {@render iconButton("lucide-list-tree", t`Show the calls below every function`, () => callStackView?.setAllChildren(true))}
            {@render iconButton("lucide-list-collapse", t`Hide the calls below every function`, () => callStackView?.setAllChildren(false))}
          </div>
          <div class="flex items-center">
            {@render iconButton("hero-arrow-up-micro", t`Previous change`, () => callStackView?.stepChange(-1), counts.changed === 0)}
            {@render iconButton("hero-arrow-down-micro", t`Next change`, () => callStackView?.stepChange(1), counts.changed === 0)}
          </div>
        </div>
      {/if}
      <div class="ml-auto flex items-center gap-3">
        {#if counts}
          <span class="font-mono text-[11px] text-base-content/50 max-sm:hidden">
            {t`${counts.changed} changed`} · {functionsCount(counts.project)}
          </span>
        {/if}
        <InfoPopover title={t`How to use this page, what its labels mean, and what was compared`} class="dropdown-end">
          <p>{t`Pick a test to walk its calls. Click a function to close its code and the calls below it, again to open them.`}</p>
          <ul class="mt-2 flex flex-col gap-1">
            {#snippet shownAbove()}<span class="font-mono text-xs">{t`shown above`}</span>{/snippet}
            <li><TranslatedParts parts={jt`${shownAbove}: already walked above, its calls are listed there`} /></li>
            {#each Object.values(STOP_LABELS) as label (label)}
              {#snippet stopLabel()}<span class="font-mono text-xs">{label}</span>{/snippet}
              <li><TranslatedParts parts={jt`${stopLabel}: the walk stopped here`} /></li>
            {/each}
          </ul>
          {@const testCount = tests.value.tests.length}
          <p class="mt-2 font-mono text-xs">
            {ngettext(msgid`${testCount} test`, `${testCount} tests`, testCount)} ·
            {shortCommit(tests.value.base_commit)} → {tests.value.head_commit ? shortCommit(tests.value.head_commit) : t`working tree`}
          </p>
          {#if selected && counts}
            {@const projectFunctions = ngettext(msgid`${counts.project} project function`, `${counts.project} project functions`, counts.project)}
            {@const systemCalls = ngettext(msgid`${counts.system} system call`, `${counts.system} system calls`, counts.system)}
            <p class="mt-2 font-mono text-xs">
              {selected.module} · {testLabel(selected)}:
              {truncated
                ? t`${counts.changed} changed, ${projectFunctions}, ${systemCalls}, cut short by the limits`
                : t`${counts.changed} changed, ${projectFunctions}, ${systemCalls}`}
            </p>
          {/if}
        </InfoPopover>
      </div>
    </Toolbar>
    <div class="full-page-body max-sm:flex-col">
      <aside
        class={[
          "flex w-80 shrink-0 flex-col border-r border-base-300",
          "max-sm:w-full max-sm:border-r-0",
          phoneTab === "calls" && "max-sm:hidden",
        ]}
      >
        <TestPicker tests={tests.value.tests} {selected} onSelect={pickTest} />
      </aside>
      <div class={["full-page-content flex flex-col max-sm:overflow-y-visible", phoneTab === "tests" && "max-sm:hidden"]}>
        {#if stack.state === "idle"}
          <p class="px-6 py-10 text-sm text-base-content/50">{t`Pick a test to walk its calls.`}</p>
        {:else if stack.state === "loading"}
          <p class="px-6 py-10 text-sm text-base-content/50">{t`Walking the calls…`}</p>
        {:else if stack.state === "failed"}
          <p class="border-b border-base-300 bg-error/10 px-4 py-3 font-mono text-xs text-error">{stack.message}</p>
        {:else}
          <CallStackView bind:this={callStackView} stack={stack.value} {systemCalls} {unchanged} />
        {/if}
      </div>
    </div>
  {/if}
</div>

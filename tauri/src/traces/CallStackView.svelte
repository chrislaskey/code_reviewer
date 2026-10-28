<!-- The calls below a test, top to bottom like a stack trace: the toolbar
     with the filters and the open and close commands, then the frames. Prev
     and Next change step through the changed functions. -->
<script>
  import { SvelteSet } from "svelte/reactivity";
  import SegmentedControl from "../lib/components/SegmentedControl.svelte";
  import Toolbar from "../lib/components/Toolbar.svelte";
  import { SHOWN_OPTIONS, frameEmphasis, isClosed, keptFrameIndexes, visibleFrames } from "./call_stack.js";
  import CallStackFrame from "./CallStackFrame.svelte";

  /** @type {{ stack: any }} */
  let { stack } = $props();

  let systemCalls = $state("hidden");
  let unchanged = $state("shown");
  const codeClosedIds = new SvelteSet();
  const childrenHiddenIds = new SvelteSet();
  let currentChange = $state(null);

  let frames = $derived(stack.frames);
  let kept = $derived(keptFrameIndexes(frames, { systemCalls, unchanged }));
  let visible = $derived(visibleFrames(frames, kept, { codeClosedIds, childrenHiddenIds }));
  let changedIndexes = $derived(visible.filter((item) => item.emphasis === "changed").map((item) => item.index));

  let changedCount = $derived(frames.filter((frame) => frameEmphasis(frame) === "changed").length);
  let systemCount = $derived(frames.filter((frame) => frameEmphasis(frame) === "system").length);
  let projectCount = $derived(frames.length - systemCount);

  // A new stack starts open, with nothing stepped to.
  $effect(() => {
    void stack;
    codeClosedIds.clear();
    childrenHiddenIds.clear();
    currentChange = null;
  });

  /** Closes the code and the children of a frame, or opens both once nothing is left to close. */
  function toggleFrame(item) {
    const open = isClosed(item);
    for (const ids of [codeClosedIds, childrenHiddenIds]) {
      if (open) ids.delete(item.index);
      else ids.add(item.index);
    }
  }

  function setAll(ids, open) {
    ids.clear();
    if (!open) frames.forEach((_, index) => ids.add(index));
  }

  function stepChange(direction) {
    if (changedIndexes.length === 0) return;
    const position = currentChange === null ? -1 : changedIndexes.indexOf(currentChange);
    const next = changedIndexes[(position + direction + changedIndexes.length) % changedIndexes.length];
    currentChange = next;
    document.getElementById(`call-stack-frame-${next}`)?.scrollIntoView({ behavior: "smooth", block: "center" });
  }

  const PAIR = "inline-flex overflow-hidden rounded-lg border border-base-300 text-xs";
  const PAIR_BUTTON = "px-3 py-1 bg-base-100 text-base-content/60 hover:text-base-content disabled:opacity-40";
</script>

{#snippet openClosePair(label, ids)}
  <div class={PAIR}>
    <button type="button" class={PAIR_BUTTON} onclick={() => setAll(ids, true)}>Open {label}</button>
    <button type="button" class={[PAIR_BUTTON, "border-l border-base-300"]} onclick={() => setAll(ids, false)}>Close {label}</button>
  </div>
{/snippet}

<Toolbar>
  <SegmentedControl label="System calls" value={systemCalls} options={SHOWN_OPTIONS} onchange={(value) => (systemCalls = value)} />
  <SegmentedControl label="Unchanged functions" value={unchanged} options={SHOWN_OPTIONS} onchange={(value) => (unchanged = value)} />
  {@render openClosePair("code", codeClosedIds)}
  {@render openClosePair("children", childrenHiddenIds)}
  <div class={PAIR}>
    <button type="button" class={PAIR_BUTTON} disabled={changedIndexes.length === 0} onclick={() => stepChange(-1)}>↑ Prev change</button>
    <button type="button" class={[PAIR_BUTTON, "border-l border-base-300"]} disabled={changedIndexes.length === 0} onclick={() => stepChange(1)}>
      Next change ↓
    </button>
  </div>
  <span class="ml-auto font-mono text-[11px] text-base-content/50">
    {changedCount} changed · {projectCount} project {projectCount === 1 ? "function" : "functions"} · {systemCount} system
    {systemCount === 1 ? "call" : "calls"}{stack.truncated ? " · cut short by the limits" : ""}
  </span>
</Toolbar>
<div class="dot-grid min-h-full p-6">
  <div class="mx-auto max-w-6xl">
    {#each visible as item (item.index)}
      <CallStackFrame visible={item} current={currentChange === item.index} ontoggle={() => toggleFrame(item)} />
    {/each}
  </div>
</div>

<!-- The calls below a test, top to bottom like a stack trace. The page's
     toolbar sets the filters, and calls `setAllCode`, `setAllChildren` and
     `stepChange`, which steps through the changed functions. -->
<script>
  import { SvelteSet } from "svelte/reactivity";
  import { isClosed, keptFrameIndexes, visibleFrames } from "./call_stack.js";
  import CallStackFrame from "./CallStackFrame.svelte";

  /** @type {{ stack: any, systemCalls: "shown" | "hidden", unchanged: "shown" | "hidden" }} */
  let { stack, systemCalls, unchanged } = $props();

  const codeClosedIds = new SvelteSet();
  const childrenHiddenIds = new SvelteSet();
  let currentChange = $state(null);

  let frames = $derived(stack.frames);
  let kept = $derived(keptFrameIndexes(frames, { systemCalls, unchanged }));
  let visible = $derived(visibleFrames(frames, kept, { codeClosedIds, childrenHiddenIds }));
  let changedIndexes = $derived(visible.filter((item) => item.emphasis === "changed").map((item) => item.index));

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

  /** Opens or closes the code of every changed function. */
  export function setAllCode(open) {
    setAll(codeClosedIds, open);
  }

  /** Shows or hides the calls below every frame. */
  export function setAllChildren(open) {
    setAll(childrenHiddenIds, open);
  }

  /** Scrolls to the next (1) or previous (-1) changed function on the page. */
  export function stepChange(direction) {
    if (changedIndexes.length === 0) return;
    const position = currentChange === null ? -1 : changedIndexes.indexOf(currentChange);
    const next = changedIndexes[(position + direction + changedIndexes.length) % changedIndexes.length];
    currentChange = next;
    document.getElementById(`call-stack-frame-${next}`)?.scrollIntoView({ behavior: "smooth", block: "center" });
  }
</script>

<div class="dot-grid min-h-full p-6">
  <div class="mx-auto max-w-6xl">
    {#each visible as item (item.index)}
      <CallStackFrame visible={item} current={currentChange === item.index} ontoggle={() => toggleFrame(item)} />
    {/each}
  </div>
</div>

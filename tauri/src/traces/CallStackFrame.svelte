<!-- One frame of the call stack, in one of three weights: a changed function
     as a card with its diff, an unchanged project function as a one-line
     card, a call outside the project as a slim row. Thin guide lines on the
     left, one for each level above it. A click closes the frame's code and
     the frames below it, or opens both again. -->
<script>
  import { CHANGE_STYLES } from "../lib/change_styles.js";
  import CodeLines from "../lib/components/CodeLines.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import { languageForPath } from "../lib/syntax_highlight.js";
  import { STOP_LABELS, calledFrom, canClose, functionLabel, isClosed } from "./call_stack.js";

  /** @type {{ visible: import("./call_stack.js").VisibleFrame, current: boolean, ontoggle: () => void }} */
  let { visible, current, ontoggle } = $props();

  const INDENT = 24;

  let frame = $derived(visible.frame);
  let change = $derived(frame.change ? CHANGE_STYLES[frame.change] : null);
  let from = $derived(calledFrom(frame));
  let closable = $derived(canClose(visible));
  let closed = $derived(isClosed(visible));
  let language = $derived(frame.file_path ? languageForPath(frame.file_path) : null);

  const BADGE = "shrink-0 rounded border px-1.5 py-px text-[10px]";
  const BADGE_PLAIN = `${BADGE} border-base-content/15 text-base-content/50`;
  const BADGE_WARN = `${BADGE} border-warning/40 text-warning`;
  const HEADER = "flex h-8 w-full items-center gap-2 px-2 text-left font-mono text-xs";
</script>

{#snippet depthGuides()}
  <div class="flex shrink-0 self-stretch">
    {#each { length: visible.depth } as _, level (level)}
      <span class="border-l border-base-content/10" style="width: {INDENT}px"></span>
    {/each}
  </div>
{/snippet}

{#snippet caret()}
  {#if closable}
    <Icon name={closed ? "hero-chevron-right-micro" : "hero-chevron-down-micro"} class="size-4 shrink-0 text-base-content/50" />
  {:else}
    <span class="size-4 shrink-0"></span>
  {/if}
{/snippet}

{#snippet badges()}
  {#if change && !frame.repeated}<span class={BADGE_WARN}>{change.label}</span>{/if}
  {#if frame.repeated}<span class={BADGE_PLAIN}>shown above</span>{/if}
  {#if frame.stop_reason}<span class={BADGE_PLAIN}>{STOP_LABELS[frame.stop_reason]}</span>{/if}
  {#if visible.skippedCallers > 0}
    <span class={BADGE_PLAIN}>via {visible.skippedCallers} unchanged {visible.skippedCallers === 1 ? "call" : "calls"}</span>
  {/if}
  {#if visible.childrenHidden && visible.childCount > 0}
    <span class="shrink-0 rounded-full bg-base-content/10 px-2 py-px text-[10px] text-base-content/60">
      {visible.childCount} hidden below
    </span>
  {/if}
{/snippet}

{#snippet trailing()}
  <span class="ml-auto flex min-w-0 shrink items-center gap-3 text-[11px] text-base-content/40">
    {#if from}<span class="truncate">{from}</span>{/if}
    {#if frame.file_path}<span class="truncate" title={frame.file_path}>{frame.file_path}</span>{/if}
  </span>
{/snippet}

<div id="call-stack-frame-{visible.index}" class="flex">
  {@render depthGuides()}
  {#if visible.emphasis === "system"}
    <div class="flex min-w-0 items-center gap-2 py-0.5 pl-2 font-mono text-[11px] text-base-content/50">
      <span class="size-4 shrink-0"></span>
      <span class="w-3 text-center">·</span>
      <span class="truncate">{functionLabel(frame.function)}</span>
      <span class={BADGE_PLAIN}>system</span>
      {#if from}<span class="truncate text-base-content/40">{from}</span>{/if}
    </div>
  {:else if visible.emphasis === "unchanged"}
    <button
      type="button"
      onclick={closable ? ontoggle : undefined}
      class={[HEADER, "my-0.5 min-w-0 flex-1 rounded-md border border-base-300 bg-base-100", closable ? "cursor-pointer" : "cursor-default"]}
    >
      {@render caret()}
      <span class="w-3 shrink-0 text-center font-bold text-base-content/30">·</span>
      <span class="truncate">
        <span class="text-base-content/60">{frame.function.module}.</span>
        <span class="font-semibold">{frame.function.name}</span>
        <span class="text-base-content/60">/{frame.function.arity}</span>
      </span>
      {@render badges()}
      {@render trailing()}
    </button>
  {:else}
    <div
      class={[
        "my-0.5 min-w-0 flex-1 rounded-r-md border border-l-2 border-base-300 bg-base-100",
        frame.change === "added" ? "border-l-success" : "border-l-warning",
        current ? "ring-2 ring-primary/60" : "",
      ]}
    >
      <button type="button" onclick={ontoggle} class={[HEADER, "cursor-pointer"]}>
        {@render caret()}
        <span class={["w-3 shrink-0 text-center font-bold", change?.text]}>{change?.sign}</span>
        <span class="truncate text-sm font-semibold">{functionLabel(frame.function)}</span>
        {@render badges()}
        {@render trailing()}
      </button>
      {#if !visible.codeClosed}
        <div class="border-t border-base-content/10">
          <CodeLines lines={frame.lines} {language} />
        </div>
      {/if}
    </div>
  {/if}
</div>

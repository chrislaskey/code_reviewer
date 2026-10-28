<!-- The tests of the head, by file, to pick one. A search box narrows them,
     and when the repository has LiveView tests a switch keeps only those. -->
<script>
  import { CHANGE_STYLES } from "../lib/change_styles.js";
  import { FIELD_CLASS } from "../lib/components/ComparisonFields.svelte";
  import { sameFunction, testLabel } from "./call_stack.js";

  /** @type {{ tests: any[], selected: any | null, onSelect: (test: any) => void }} */
  let { tests, selected, onSelect } = $props();

  let hasLiveViewTests = $derived(tests.some((test) => test.is_live_view_test));
  let liveViewOnly = $state(false);
  let search = $state("");

  let shown = $derived.by(() => {
    const needle = search.trim().toLowerCase();
    return tests.filter(
      (test) =>
        (!liveViewOnly || test.is_live_view_test) &&
        (needle === "" ||
          `${test.file_path} ${test.function.module} ${test.function.name}`.toLowerCase().includes(needle)),
    );
  });

  let byFile = $derived.by(() => {
    const groups = new Map();
    for (const test of shown) {
      const group = groups.get(test.file_path);
      if (group) group.push(test);
      else groups.set(test.file_path, [test]);
    }
    return [...groups].map(([filePath, tests]) => ({ filePath, tests }));
  });
</script>

<div class="space-y-2 border-b border-base-300 p-3">
  <input bind:value={search} placeholder="Find a test" spellcheck="false" class={["w-full", FIELD_CLASS]} />
  {#if hasLiveViewTests}
    <label class="flex items-center gap-2 text-[11px] text-base-content/60">
      <input type="checkbox" bind:checked={liveViewOnly} />
      LiveView tests only
    </label>
  {/if}
  <p class="font-mono text-[11px] text-base-content/50">{shown.length} of {tests.length} tests</p>
</div>
<div class="min-h-0 flex-1 overflow-auto">
  {#each byFile as group (group.filePath)}
    <div class="border-b border-base-300 py-1">
      <p class="truncate px-3 py-1 font-mono text-[10px] text-base-content/50" style="direction: rtl" title={group.filePath}>
        {group.filePath}
      </p>
      {#each group.tests as test (`${test.function.module}:${test.function.name}/${test.function.arity}`)}
        {@const isSelected = selected !== null && sameFunction(selected, test.function)}
        {@const change = test.change ? CHANGE_STYLES[test.change] : null}
        <button
          type="button"
          onclick={() => onSelect(test)}
          class={[
            "flex w-full items-center gap-2 px-3 py-1 text-left font-mono text-[11px]",
            isSelected ? "bg-base-content/10 text-base-content" : "text-base-content/80 hover:bg-base-content/5",
          ]}
        >
          <span
            class={["size-1.5 shrink-0 rounded-full", change ? change.dot : "bg-base-content/20"]}
            title={change ? `test ${change.label}` : "test unchanged"}
          ></span>
          <span class="truncate">{testLabel(test.function)}</span>
        </button>
      {/each}
    </div>
  {:else}
    <p class="px-3 py-4 text-[11px] text-base-content/50">No tests match.</p>
  {/each}
</div>

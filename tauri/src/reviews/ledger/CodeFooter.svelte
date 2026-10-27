<!-- The code under the ledger of a parsed file: the whole of every changed
     function, with the changed lines marked, in one block. Closed at first.
     Hovering a function in the ledger shows only its code; a click opens or
     closes all of it. The code comes from the backend, one request per file. -->
<script>
  import { getContext } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import { plural } from "../change_styles.js";
  import CodeLines from "./CodeLines.svelte";
  import { followOpenAllCommand } from "./open_all.svelte.js";
  import { languageForPath } from "./syntax_highlight.js";

  /** @type {{ path: string, functions: any[], hoveredKey: string | null }} */
  let { path, functions, hoveredKey } = $props();

  const loaders = getContext("review-code");

  /** @typedef {{ state: "loading" } | { state: "failed", message: string } | { state: "ready", items: any[] }} Loaded */
  let loaded = $state(/** @type {Loaded} */ ({ state: "loading" }));
  $effect(() => {
    const targets = functions.map((fn) => fn.diff_target);
    let outdated = false;
    loaded = { state: "loading" };
    loaders
      .loadFunctionDiffs(targets)
      .then((items) => {
        if (!outdated) loaded = { state: "ready", items };
      })
      .catch((error) => {
        if (!outdated) loaded = { state: "failed", message: String(error) };
      });
    return () => {
      outdated = true;
    };
  });

  const pinned = followOpenAllCommand(false);
  let hoveredIndex = $derived(functions.findIndex((fn) => fn.key === hoveredKey));
  let previewing = $derived(hoveredIndex >= 0);
  let open = $derived(pinned.open || previewing);
  let shown = $derived(previewing ? [hoveredIndex] : functions.map((_, index) => index));
  let language = $derived(languageForPath(path));
</script>

<div class="border-t border-base-content/10">
  <div class="flex h-5 items-center px-3 pt-2 text-[10px] uppercase tracking-wide text-base-content/40">Code</div>
  {#if loaded.state === "loading"}
    <p class="px-3 py-2 font-mono text-[11px] text-base-content/50">Loading code…</p>
  {:else if loaded.state === "failed"}
    <p class="px-3 py-2 font-mono text-[11px] text-error">{loaded.message}</p>
  {:else}
    <button
      type="button"
      onclick={() => (pinned.open = !pinned.open)}
      class={["flex h-8 w-full cursor-pointer items-center gap-2 px-3 text-left font-mono text-xs", previewing ? "bg-base-content/5" : ""]}
    >
      <Icon name="hero-chevron-right-micro" class={["size-3.5 text-base-content/50 transition-transform", open ? "rotate-90" : ""]} />
      <span class="text-base-content/70">{plural(functions.length, "changed function")}</span>
    </button>
    {#if open}
      {#each shown as index (index)}
        <div class="border-t border-base-content/10 first:border-t-0">
          <CodeLines lines={loaded.items[index]?.lines ?? []} {language} />
        </div>
      {/each}
    {/if}
  {/if}
</div>

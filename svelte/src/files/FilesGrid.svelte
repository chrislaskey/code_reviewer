<!-- The files index's grid: a square for each of `total` files, as many
     to a row as fit, drawing only those on screen and a screen above and
     below. It is as tall as every row, so the page scrolls as if all
     were there. A square shows its file's image, or an icon when it is
     not one, and a mark when it is not on the server yet; an empty
     square is a file whose page has not come yet. It calls `onshow`
     with the positions it draws as they change, for the page to fetch
     the files there. See docs/design/files-browsing.md. -->
<script>
  import FileImage from "../lib/FileImage.svelte";
  import { filesContentUrl } from "../lib/core/index.js";
  import { filesShowUrl } from "../lib/routes.js";
  import { Icon } from "../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {number} total
   * @property {any[]} rows the files by position, a hole where one has not come yet
   * @property {(start: number, end: number) => void} onshow
   */

  /** @type {Props} */
  let { total, rows, onshow } = $props();

  const SMALLEST_TILE = 72;
  const GAP = 8;

  /** @type {HTMLDivElement | undefined} */
  let grid = $state();
  let width = $state(0);
  let firstRow = $state(0);
  let lastRow = $state(0);

  let columns = $derived(Math.max(1, Math.floor((width + GAP) / (SMALLEST_TILE + GAP))));
  let tile = $derived(Math.max(0, (width - GAP * (columns - 1)) / columns));
  let rowHeight = $derived(tile + GAP);
  let rowCount = $derived(Math.ceil(total / columns));
  let height = $derived(Math.max(0, rowCount * rowHeight - GAP));
  let first = $derived(firstRow * columns);
  let last = $derived(Math.min(total, lastRow * columns));
  let positions = $derived(Array.from({ length: Math.max(0, last - first) }, (_, offset) => first + offset));

  function measure() {
    if (!grid || rowHeight <= GAP) return;
    const top = grid.getBoundingClientRect().top;
    const screenRows = Math.ceil(window.innerHeight / rowHeight);
    firstRow = Math.max(0, Math.floor(-top / rowHeight) - screenRows);
    lastRow = Math.min(rowCount, Math.ceil((window.innerHeight - top) / rowHeight) + screenRows);
  }

  $effect(() => {
    if (!grid) return;
    const observer = new ResizeObserver(([entry]) => (width = entry.contentRect.width));
    observer.observe(grid);
    return () => observer.disconnect();
  });

  $effect(() => {
    let frame = 0;
    const measureNextFrame = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    };
    window.addEventListener("scroll", measureNextFrame, { passive: true });
    window.addEventListener("resize", measureNextFrame);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", measureNextFrame);
      window.removeEventListener("resize", measureNextFrame);
    };
  });

  // Measures again when the rows' size or count changes.
  $effect(() => {
    void rowHeight;
    void rowCount;
    measure();
  });

  $effect(() => {
    if (last > first) onshow(first, last);
  });
</script>

<div bind:this={grid} class="relative" style:height="{height}px">
  <ul
    class="absolute inset-x-0 grid"
    style:top="{firstRow * rowHeight}px"
    style:gap="{GAP}px"
    style:grid-template-columns="repeat({columns}, minmax(0, 1fr))"
    style:grid-auto-rows="{tile}px"
  >
    {#each positions as position (`${position}:${rows[position]?.file.id ?? ""}`)}
      {@const shown = rows[position]}
      <li>
        {#if shown}
          {@const { file, shown_as, state } = shown}
          <a
            href={filesShowUrl(file.id)}
            title={file.name}
            class="relative flex size-full items-center justify-center rounded-box bg-base-200 p-1 hover:bg-base-300"
          >
            {#if shown_as === "image"}
              <FileImage src={filesContentUrl(file.id)} alt={file.name} class="size-full object-contain" missing="" />
            {:else}
              <Icon name="hero-document-text" class="size-8 opacity-60" />
              <span class="sr-only">{file.name}</span>
            {/if}
            {#if state === "waiting"}
              <Icon name="hero-cloud-arrow-up-mini" class="absolute top-1 right-1 size-4" />
              <span class="sr-only">Waiting to upload</span>
            {:else if state === "refused"}
              <Icon name="hero-exclamation-triangle-mini" class="absolute top-1 right-1 size-4 text-warning" />
              <span class="sr-only">Not saved on the server</span>
            {/if}
          </a>
        {:else}
          <div class="size-full rounded-box bg-base-200"></div>
        {/if}
      </li>
    {/each}
  </ul>
</div>

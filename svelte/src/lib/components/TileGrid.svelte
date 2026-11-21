<!-- A grid of `total` square tiles, as many to a row as fit, drawing only
     those on screen and a screen above and below. It is as tall as every
     row, so its scroller, the window or the element given, scrolls as if
     all were there. `tile` draws a position's row, or nothing yet when
     its page has not come, an empty square. It calls `onshow` with the
     positions it draws as they change, for the page to fetch the rows
     there. Used by the files index and the quick picker, see
     docs/design/files-browsing.md. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {number} total
   * @property {any[]} rows a files search's, by position, a hole where one has not come yet
   * @property {(start: number, end: number) => void} onshow
   * @property {import("svelte").Snippet<[any, number]>} tile a row and its position
   * @property {HTMLElement} [scroller] the element that scrolls, the window when none
   * @property {number} [columns] bindable: how many tiles to a row
   */

  /** @type {Props} */
  let { total, rows, onshow, tile, scroller, columns = $bindable(1) } = $props();

  const SMALLEST_TILE = 72;
  const GAP = 8;

  /** @type {HTMLDivElement | undefined} */
  let grid = $state();
  let width = $state(0);
  let firstRow = $state(0);
  let lastRow = $state(0);

  $effect(() => {
    columns = Math.max(1, Math.floor((width + GAP) / (SMALLEST_TILE + GAP)));
  });
  let tileSize = $derived(Math.max(0, (width - GAP * (columns - 1)) / columns));
  let rowHeight = $derived(tileSize + GAP);
  let rowCount = $derived(Math.ceil(total / columns));
  let height = $derived(Math.max(0, rowCount * rowHeight - GAP));
  let first = $derived(firstRow * columns);
  let last = $derived(Math.min(total, lastRow * columns));
  let positions = $derived(Array.from({ length: Math.max(0, last - first) }, (_, offset) => first + offset));

  /** The scroller's top and height on screen. */
  function view() {
    if (!scroller) return { top: 0, height: window.innerHeight };
    const { top, height } = scroller.getBoundingClientRect();
    return { top, height };
  }

  function measure() {
    if (!grid || rowHeight <= GAP) return;
    const shown = view();
    const top = grid.getBoundingClientRect().top - shown.top;
    const screenRows = Math.ceil(shown.height / rowHeight);
    firstRow = Math.max(0, Math.floor(-top / rowHeight) - screenRows);
    lastRow = Math.min(rowCount, Math.ceil((shown.height - top) / rowHeight) + screenRows);
  }

  /**
   * Scrolls the least that brings the tile at `position` on screen.
   * @param {number} position
   */
  export function scrollToPosition(position) {
    if (!grid || rowHeight <= GAP) return;
    const shown = view();
    const top = grid.getBoundingClientRect().top - shown.top + Math.floor(position / columns) * rowHeight;
    const bottom = top + tileSize;
    const by = top < 0 ? top : bottom > shown.height ? bottom - shown.height : 0;
    if (by) (scroller ?? window).scrollBy({ top: by });
  }

  $effect(() => {
    if (!grid) return;
    const observer = new ResizeObserver(([entry]) => (width = entry.contentRect.width));
    observer.observe(grid);
    return () => observer.disconnect();
  });

  $effect(() => {
    const scrolled = scroller ?? window;
    let frame = 0;
    const measureNextFrame = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    };
    scrolled.addEventListener("scroll", measureNextFrame, { passive: true });
    window.addEventListener("resize", measureNextFrame);
    return () => {
      cancelAnimationFrame(frame);
      scrolled.removeEventListener("scroll", measureNextFrame);
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
    style:grid-auto-rows="{tileSize}px"
  >
    {#each positions as position (`${position}:${rows[position]?.file.id ?? ""}`)}
      <li>
        {#if rows[position]}
          {@render tile(rows[position], position)}
        {:else}
          <div class="size-full rounded-box bg-base-200"></div>
        {/if}
      </li>
    {/each}
  </ul>
</div>

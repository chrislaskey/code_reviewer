<!-- The app's navigation on small windows, where the gutter is hidden: a
     bar along the window's bottom edge with a tile per page, as many as
     fit beside a last tile that drops up a menu of the theme choices: Theme
     while every page fits, else More, with the pages that don't above them. Drawn like the gutter: the open tab joined to the page
     above it, the gutter's shape turned a quarter so the page's edge runs
     along the top. The last tile is a details element, like ChoiceMenu, closing on
     a click outside it and on Escape. `links` and `current` are AppNav's. -->
<script>
  import Icon from "./Icon.svelte";
  import ThemeChoices from "./ThemeChoices.svelte";
  import { folderTabPath } from "../folder_tab.js";

  /**
   * @typedef {Object} Props
   * @property {{ id: string, label: string, icon: string, href: string }[]} links
   * @property {string} current
   */

  /** @type {Props} */
  let { links, current } = $props();

  // Five tiles fit the narrowest window (320px); the last is Theme or More.
  const TILES_IN_BAR = 5;

  let linksInBar = $derived(links.slice(0, TILES_IN_BAR - 1));
  let linksInMore = $derived(links.slice(TILES_IN_BAR - 1));
  let currentIsInMore = $derived(linksInMore.some((link) => link.id === current));
  let hasMore = $derived(linksInMore.length > 0);

  // The gutter's (AppNav).
  const OVERLAP = 2;
  const CORNER_RADIUS = 10;
  const FEET_RADIUS = 10;

  let open = $state(false);
  let element = $state();
  let width = $state(0);
  let height = $state(0);
  /** @type {Record<string, HTMLElement>} */
  let tiles = $state({});

  /* The gutter's shape, measured across the bar from its bottom edge and
     along it from its left, then turned into place (turnIntoBar). More's
     tile is measured by its details element, the bar's child. */
  let shape = $derived.by(() => {
    if (height === 0) return "";
    const tile = currentIsInMore ? element : tiles[current];
    const tab = tile ? { left: 0, top: tile.offsetLeft, bottom: tile.offsetLeft + tile.offsetWidth } : null;
    return folderTabPath({
      width: height + OVERLAP,
      height: width,
      edge: height,
      tab,
      cornerRadius: CORNER_RADIUS,
      feetRadius: FEET_RADIUS,
    });
  });

  // A quarter turn: the gutter's x, from the bar's bottom, runs up the bar.
  let turnIntoBar = $derived(`matrix(0 -1 1 0 0 ${height + OVERLAP})`);

  function close() {
    open = false;
  }

  function closeWhenClickedOutside(event) {
    if (open && element && !element.contains(event.target)) open = false;
  }

  function closeOnEscape(event) {
    if (event.key === "Escape") open = false;
  }
</script>

<svelte:window onclick={closeWhenClickedOutside} onkeydown={closeOnEscape} />

<nav class="bottom-bar" bind:clientWidth={width} bind:clientHeight={height}>
  <svg
    class="tab-shape"
    {width}
    height={height + OVERLAP}
    style="top: -{OVERLAP}px; clip-path: inset({OVERLAP}px 0 0 0)"
    aria-hidden="true"
  >
    <path d={shape} transform={turnIntoBar} />
  </svg>
  {#each linksInBar as link (link.id)}
    <a href={link.href} class="tile" class:current={current === link.id} bind:this={tiles[link.id]}>
      <Icon name={link.icon} class="tile-icon" />
      {link.label}
    </a>
  {/each}
  <details class="dropdown dropdown-top dropdown-end h-full flex-1" bind:open bind:this={element}>
    <summary class="tile" class:current={currentIsInMore}>
      <Icon name={hasMore ? "hero-ellipsis-horizontal" : "hero-swatch"} class="tile-icon" />
      {hasMore ? "More" : "Theme"}
    </summary>
    <ul class="dropdown-content menu panel mb-1 max-h-[70vh] w-52 flex-nowrap overflow-y-auto">
      {#each linksInMore as link (link.id)}
        <li>
          <a href={link.href} class="text-base-content no-underline" class:menu-active={current === link.id} onclick={close}>
            <Icon name={link.icon} class="size-4" />
            {link.label}
          </a>
        </li>
      {/each}
      {#if hasMore}<li class="menu-title">Theme</li>{/if}
      <ThemeChoices onchoose={close} />
    </ul>
  </details>
</nav>

<style>
  /* Fixed along the bottom, like the gutter down the left, in its colour. */
  .bottom-bar {
    position: fixed;
    inset: auto 0 0 0;
    z-index: 20;
    display: none;
    height: var(--bottom-bar-height);
    background: var(--gutter-bg);
  }

  @media (width < 40rem) {
    .bottom-bar {
      display: flex;
    }
  }

  /* The gutter's tab shape (styles.css), its ring and shadow turned with
     it: they fall below the page's edge instead of left of it. */
  .tab-shape {
    position: absolute;
    left: 0;
    z-index: 0;
    overflow: visible;
    pointer-events: none;
    fill: var(--color-base-100);
    filter: drop-shadow(0 1px 0 var(--gutter-ring)) drop-shadow(-1px 0 0 var(--gutter-ring))
      drop-shadow(1px 0 0 var(--gutter-ring)) drop-shadow(1px 3px 7px var(--gutter-shadow));
  }

  /* A tile: the gutter's, an icon over one word, sharing the bar evenly. */
  .tile {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.25rem;
    height: 100%;
    list-style: none;
    color: color-mix(in oklab, var(--color-base-content) 62%, transparent);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.01em;
    text-decoration: none;
    cursor: pointer;
  }

  summary.tile {
    width: 100%;
  }

  summary.tile::-webkit-details-marker {
    display: none;
  }

  .tile :global(.tile-icon) {
    width: 22px;
    height: 22px;
  }

  /* As in the gutter, a tab that is not open lights up as an inset
     rounded square, while hovered or while its menu is open. */
  .tile:not(.current):hover,
  details[open] > .tile:not(.current),
  .tile.current {
    color: var(--fg);
  }

  .tile:not(.current):hover::before,
  details[open] > .tile:not(.current)::before {
    content: "";
    position: absolute;
    inset: 4px 7px;
    z-index: -1;
    border-radius: 10px;
    background: color-mix(in oklab, var(--color-base-content) 7%, transparent);
  }

  .tile.current :global(.tile-icon) {
    color: var(--color-primary);
  }
</style>

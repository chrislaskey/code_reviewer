<!-- The app's navigation: a gutter down the window's left edge with a tab
     per page, the open one joined to the page like a file folder's tab, and
     the theme menu as a tile at the bottom. The macOS window controls sit
     at its top, over the room left for them, and the empty parts of the
     gutter and the title bar's strip over the page drag the window.
     `current` names the tab the page belongs to. -->
<script>
  import Icon from "./Icon.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";
  import { folderTabPath } from "../folder_tab.js";
  import { homeUrl, reviewNewUrl, sessionsIndexUrl, traceNewUrl } from "../routes.js";

  /** @type {{ current?: "home" | "reviews" | "traces" | "sessions" }} */
  let { current = "home" } = $props();

  const LINKS = [
    { id: "home", label: "Home", icon: "hero-home", href: homeUrl() },
    { id: "sessions", label: "Sessions", icon: "hero-command-line", href: sessionsIndexUrl() },
    { id: "reviews", label: "Review", icon: "hero-document-magnifying-glass", href: reviewNewUrl() },
    { id: "traces", label: "Trace", icon: "hero-beaker", href: traceNewUrl() },
  ];

  /* The shape spills this far under the page, so the two never show a seam;
     it is clipped there, so it never covers the page. */
  const OVERLAP = 2;
  const CORNER_RADIUS = 10;
  const FEET_RADIUS = 10;

  let width = $state(0);
  let height = $state(0);
  /** @type {Record<string, HTMLElement>} */
  let tiles = $state({});

  // Measured again whenever the gutter is: on mount and on every resize.
  let shape = $derived.by(() => {
    if (height === 0) return "";
    const tile = tiles[current];
    const tab = tile ? { left: tile.offsetLeft, top: tile.offsetTop, bottom: tile.offsetTop + tile.offsetHeight } : null;
    return folderTabPath({
      width: width + OVERLAP,
      height,
      edge: width,
      tab,
      cornerRadius: CORNER_RADIUS,
      feetRadius: FEET_RADIUS,
    });
  });
</script>

<nav class="app-gutter" bind:clientWidth={width} bind:clientHeight={height}>
  <svg class="tab-shape" width={width + OVERLAP} {height} style="clip-path: inset(0 {OVERLAP}px 0 0)" aria-hidden="true">
    <path d={shape} />
  </svg>
  <div class="window-controls" data-tauri-drag-region></div>
  {#each LINKS as link (link.id)}
    <a href={link.href} class="tile" class:current={current === link.id} bind:this={tiles[link.id]}>
      <Icon name={link.icon} class="tile-icon" />
      {link.label}
    </a>
  {/each}
  <div class="spacer" data-tauri-drag-region></div>
  <ThemeMenu />
</nav>

<div class="title-bar-drag" data-tauri-drag-region></div>

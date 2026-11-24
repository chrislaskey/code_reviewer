<!-- The app's navigation: a gutter down the window's left edge with a tab
     per part of the app, the open one joined to the page like a file
     folder's tab, and the theme menu as a tile at the bottom, below the
     app's header (AppHeader). On the desktop its empty parts drag the
     window. On small windows the gutter is hidden and BottomBar
     takes its place. A visitor sees Home only; staff on the web see Ops,
     another entry, which the router leaves to the browser (`target`).
     The open tab is the one whose path the page's starts with. -->
<script>
  import { t } from "ttag";
  import BottomBar from "./BottomBar.svelte";
  import Icon from "../components/Icon.svelte";
  import ThemeMenu from "./ThemeMenu.svelte";
  import { folderTabPath } from "./folder_tab.js";
  import { currentPathname } from "../router.js";
  import {
    communitiesIndexUrl,
    forumIndexUrl,
    homeUrl,
    opsHomeUrl,
    supportIndexUrl,
  } from "../routes.js";
  import { currentUser } from "../current_user.svelte.js";

  /** @typedef {{ id: string, label: string, icon: string, href: string, target?: string }} Link */


  /** @type {Link[]} */
  let links = $derived([
    { id: "home", label: t`Home`, icon: "hero-home", href: homeUrl() },
    ...(currentUser.value
      ? [
          { id: "communities", label: t`Communities`, icon: "hero-user-group", href: communitiesIndexUrl() },
          { id: "support", label: t`Support`, icon: "hero-lifebuoy", href: supportIndexUrl() },
          { id: "forum", label: t`Forum`, icon: "hero-chat-bubble-left-right", href: forumIndexUrl() },
        ]
      : []),
    ...(currentUser.value?.is_staff
      ? [{ id: "ops", label: t`Ops`, icon: "hero-wrench-screwdriver", href: opsHomeUrl(), target: "_self" }]
      : []),
  ]);

  let current = $derived(
    links.find(({ href }) => currentPathname() === href || currentPathname().startsWith(`${href}/`))?.id ?? "",
  );

  /* The shape spills this far under the page, so the two never show a seam;
     it is clipped there, so it never covers the page. */
  const OVERLAP = 2;
  const CORNER_RADIUS = 10;
  const FEET_RADIUS = 10;

  let width = $state(0);
  let height = $state(0);
  /** @type {Record<string, HTMLElement>} */
  let tiles = $state({});

  // Measured again whenever the gutter is, or the open tab changes.
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

<nav class="app-gutter" aria-label={t`Main`} bind:clientWidth={width} bind:clientHeight={height}>
  <svg class="tab-shape" width={width + OVERLAP} {height} style="clip-path: inset(0 {OVERLAP}px 0 0)" aria-hidden="true">
    <path d={shape} />
  </svg>
  <div class="gutter-top"></div>
  {#each links as link (link.id)}
    <a href={link.href} target={link.target} class="tile" class:current={current === link.id} bind:this={tiles[link.id]}>
      <Icon name={link.icon} class="tile-icon" />
      {link.label}
    </a>
  {/each}
  <div class="spacer" data-tauri-drag-region></div>
  <ThemeMenu />
</nav>

<BottomBar {links} {current} />

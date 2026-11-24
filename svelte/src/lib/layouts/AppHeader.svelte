<!-- The app's header, across the window's top above the gutter (AppNav)
     and the page: in the desktop app Back and Forward beside the window
     controls, which it has no browser's buttons for; the current
     community to switch, for someone signed in on a server with more
     than one community; then on the right the
     sync status (SyncMenu) and the person (AccountMenu). Its empty parts
     drag the window on the desktop. On small windows Back and Forward are
     the bottom bar's (BottomBar). -->
<script>
  import { t } from "ttag";
  import AccountMenu from "./AccountMenu.svelte";
  import CurrentCommunityMenu from "./CurrentCommunityMenu.svelte";
  import SyncMenu from "./SyncMenu.svelte";
  import { Button, Icon } from "../components/index.js";
  import {
    goBackInHistory,
    goForwardInHistory,
    navigationHistory,
    trackNavigationHistory,
  } from "./navigation_history.svelte.js";
  import { currentUser } from "../current_user.svelte.js";
  import { currentServerHasSingleCommunity } from "../current_server.svelte.js";


  $effect(() => trackNavigationHistory());
</script>

<header class="app-header" data-tauri-drag-region>
  <div class="page-corner" aria-hidden="true"></div>
  <div class="app-header-history">
    <Button
      variant="ghost"
      size="sm"
      class="btn-square"
      aria-label={t`Back`}
      title={t`Back`}
      disabled={!navigationHistory.canGoBack}
      onclick={goBackInHistory}
    >
      <Icon name="hero-chevron-left" class="size-5" />
    </Button>
    <Button
      variant="ghost"
      size="sm"
      class="btn-square"
      aria-label={t`Forward`}
      title={t`Forward`}
      disabled={!navigationHistory.canGoForward}
      onclick={goForwardInHistory}
    >
      <Icon name="hero-chevron-right" class="size-5" />
    </Button>
  </div>
  {#if currentUser.value && !currentServerHasSingleCommunity()}
    <CurrentCommunityMenu />
  {/if}
  <span class="grow self-stretch" data-tauri-drag-region></span>
  {#if currentUser.value}
    <SyncMenu />
  {/if}
  <AccountMenu />
</header>

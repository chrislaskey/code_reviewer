<!-- The app's navigation on small windows, where the gutter is hidden: a
     bar along the window's bottom edge with Home, Back, Forward and Menu.
     Menu slides up a sheet with the other tabs and the theme choices; it
     closes on Menu, a tap above it, a choice and Escape. `links` and
     `current` are AppNav's; `current` marks the open tab in the sheet. -->
<script>
  import { t } from "ttag";
  import { fade, fly } from "svelte/transition";
  import Icon from "./components/Icon.svelte";
  import ThemeChoices from "./ThemeChoices.svelte";
  import {
    goBackInHistory,
    goForwardInHistory,
    navigationHistory,
    trackNavigationHistory,
  } from "./navigation_history.svelte.js";

  /** @type {{ links: { id: string, label: string, icon: string, href: string, target?: string }[], current: string }} */
  let { links, current } = $props();

  let homeLink = $derived(links[0]);
  let linksInMenu = $derived(links.slice(1));

  let open = $state(false);

  function close() {
    open = false;
  }

  function closeOnEscape(event) {
    if (event.key === "Escape") open = false;
  }

  $effect(() => trackNavigationHistory());
</script>

<svelte:window onkeydown={closeOnEscape} />

{#if open}
  <button type="button" class="sheet-backdrop" aria-label={t`Close menu`} onclick={close} transition:fade={{ duration: 150 }}
  ></button>
  <div class="sheet" role="dialog" aria-label={t`Menu`} transition:fly={{ y: 40, duration: 200 }}>
    {#if linksInMenu.length > 0}
      <ul class="sheet-links">
        {#each linksInMenu as link (link.id)}
          <li>
            <a href={link.href} target={link.target} class="sheet-link" class:current={current === link.id} onclick={close}>
              <Icon name={link.icon} class="size-6" />
              {link.label}
            </a>
          </li>
        {/each}
      </ul>
    {/if}
    <h2 class="sheet-heading">{t`Theme`}</h2>
    <ul class="menu w-full p-0">
      <ThemeChoices onchoose={close} />
    </ul>
  </div>
{/if}

<nav class="bottom-bar" aria-label={t`Main`}>
  <a href={homeLink.href} class="bar-button" onclick={close}>
    <Icon name={homeLink.icon} class="bar-icon" />
    {homeLink.label}
  </a>
  <button type="button" class="bar-button" disabled={!navigationHistory.canGoBack} onclick={goBackInHistory}>
    <Icon name="hero-chevron-left" class="bar-icon" />
    {t`Back`}
  </button>
  <button type="button" class="bar-button" disabled={!navigationHistory.canGoForward} onclick={goForwardInHistory}>
    <Icon name="hero-chevron-right" class="bar-icon" />
    {t`Forward`}
  </button>
  <button type="button" class="bar-button" aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name={open ? "hero-x-mark" : "hero-bars-3"} class="bar-icon" />
    {t`Menu`}
  </button>
</nav>

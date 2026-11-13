<!-- The app's navigation on small windows, where the gutter is hidden: a
     bar along the window's bottom edge with four buttons, Home, Back,
     Forward and More. More slides up a sheet over most of the window with
     the other pages and the theme choices. The sheet closes on More, a tap
     above it, a choice and Escape. `links` and `current` are AppNav's;
     `current` marks the open page in the sheet. -->
<script>
  import { fade, fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import ThemeChoices from "./ThemeChoices.svelte";
  import { goBackInHistory, goForwardInHistory, navigationHistory } from "../navigation_history.svelte.js";

  /**
   * @typedef {Object} Props
   * @property {{ id: string, label: string, icon: string, href: string }[]} links
   * @property {string} current
   */

  /** @type {Props} */
  let { links, current } = $props();

  let homeLink = $derived(links.find((link) => link.id === "home"));
  let linksInMore = $derived(links.filter((link) => link.id !== "home"));

  let open = $state(false);

  function close() {
    open = false;
  }

  function closeOnEscape(event) {
    if (event.key === "Escape") open = false;
  }
</script>

<svelte:window onkeydown={closeOnEscape} />

{#if open}
  <button type="button" class="sheet-backdrop" aria-label="Close menu" onclick={close} transition:fade={{ duration: 150 }}
  ></button>
  <div class="sheet" role="dialog" aria-label="More" transition:fly={{ y: 40, duration: 200 }}>
    <ul class="page-links">
      {#each linksInMore as link (link.id)}
        <li>
          <a href={link.href} class="page-link" class:current={current === link.id} onclick={close}>
            <Icon name={link.icon} class="size-6" />
            {link.label}
          </a>
        </li>
      {/each}
    </ul>
    <h2 class="sheet-heading">Theme</h2>
    <ul class="menu w-full p-0">
      <ThemeChoices onchoose={close} />
    </ul>
  </div>
{/if}

<nav class="bottom-bar">
  <a href={homeLink?.href} class="bar-button" onclick={close}>
    <Icon name={homeLink?.icon ?? "hero-home"} class="bar-icon" />
    Home
  </a>
  <button type="button" class="bar-button" disabled={!navigationHistory.canGoBack} onclick={goBackInHistory}>
    <Icon name="hero-chevron-left" class="bar-icon" />
    Back
  </button>
  <button type="button" class="bar-button" disabled={!navigationHistory.canGoForward} onclick={goForwardInHistory}>
    <Icon name="hero-chevron-right" class="bar-icon" />
    Forward
  </button>
  <button type="button" class="bar-button" class:open aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name={open ? "hero-x-mark" : "hero-bars-3"} class="bar-icon" />
    More
  </button>
</nav>

<style>
  /* Fixed along the bottom in the gutter's colour, a hairline above it. */
  .bottom-bar {
    position: fixed;
    inset: auto 0 0 0;
    z-index: 20;
    display: none;
    gap: 6px;
    height: var(--bottom-bar-height);
    padding: 6px 8px;
    background: var(--gutter-bg);
    box-shadow: 0 -1px 0 var(--gutter-ring);
  }

  @media (width < 40rem) {
    .bottom-bar {
      display: flex;
    }
  }

  /* A button: a raised rounded square, an icon over one word, sharing the
     bar evenly; it sinks while pressed. */
  .bar-button {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    border-radius: 10px;
    background: var(--color-base-100);
    box-shadow: var(--raised-shadow);
    color: var(--fg);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.01em;
    text-decoration: none;
    cursor: pointer;
    transition:
      transform 80ms,
      background-color 80ms;
  }

  .bar-button:not(:disabled):active {
    transform: scale(0.96);
    background: color-mix(in oklab, var(--color-base-content) 8%, var(--color-base-100));
  }

  .bar-button:disabled {
    box-shadow: none;
    background: transparent;
    color: color-mix(in oklab, var(--color-base-content) 30%, transparent);
    cursor: default;
  }

  .bar-button.open {
    background: color-mix(in oklab, var(--color-primary) 14%, var(--color-base-100));
  }

  .bar-button.open :global(.bar-icon) {
    color: var(--color-primary);
  }

  .bar-button :global(.bar-icon) {
    width: 20px;
    height: 20px;
  }

  /* The page above the sheet, dimmed; a tap on it closes the sheet. */
  .sheet-backdrop {
    position: fixed;
    inset: 0 0 var(--bottom-bar-height) 0;
    z-index: 18;
    background: rgb(0 0 0 / 0.35);
  }

  /* Rises from the bar over most of the window, below the status bar on a
     phone (the safe area); scrolls when the theme list runs long. */
  .sheet {
    position: fixed;
    inset: auto 0 var(--bottom-bar-height) 0;
    z-index: 19;
    max-height: calc(100% - var(--bottom-bar-height) - env(safe-area-inset-top, 0px) - 3rem);
    overflow-y: auto;
    padding: 1.25rem var(--page-gutter) 1rem;
    border-radius: 16px 16px 0 0;
    background: var(--color-base-100);
    box-shadow: var(--raised-shadow);
  }

  .page-links {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.5rem;
  }

  /* A page: an icon over its name, a large touch target. */
  .page-link {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.375rem;
    padding: 1rem 0.5rem;
    border-radius: 12px;
    background: var(--gutter-bg);
    color: var(--fg);
    font-weight: 600;
    text-decoration: none;
  }

  .page-link.current {
    outline: 2px solid var(--color-primary);
  }

  .page-link.current :global(.size-6) {
    color: var(--color-primary);
  }

  .sheet-heading {
    margin: 1.25rem 0 0.25rem;
    padding: 0 0.75rem;
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 600;
  }
</style>

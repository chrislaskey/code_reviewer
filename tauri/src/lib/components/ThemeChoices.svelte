<!-- A menu's choices of theme: "System", to follow the OS light or dark
     setting, then every daisyUI theme. A choice dispatches the
     `phx:set-theme` event the theme script in each page's <head> listens
     for (src/lib/theme_script.js), then calls `onchoose`. -->
<script>
  import Icon from "./Icon.svelte";
  import { THEMES, currentTheme } from "../themes.js";

  /**
   * @typedef {Object} Props
   * @property {string} [current]
   * @property {() => void} [onchoose]
   */

  /** @type {Props} */
  let { current = $bindable(currentTheme()), onchoose } = $props();

  function setTheme(event) {
    event.currentTarget.dispatchEvent(new CustomEvent("phx:set-theme", { bubbles: true, cancelable: true }));
    current = currentTheme();
    onchoose?.();
  }
</script>

<li>
  <button type="button" class:menu-active={current === "system"} onclick={setTheme} data-phx-theme="system">
    <Icon name="hero-computer-desktop-micro" class="size-4" />
    System
  </button>
</li>
{#each THEMES as theme (theme)}
  <li>
    <button type="button" class:menu-active={current === theme} onclick={setTheme} data-phx-theme={theme}>
      <span data-theme={theme} class="grid grid-cols-2 gap-0.5 rounded-sm bg-base-100 p-0.5 shadow-sm">
        <span class="size-1.5 rounded-full bg-base-content"></span>
        <span class="size-1.5 rounded-full bg-primary"></span>
        <span class="size-1.5 rounded-full bg-secondary"></span>
        <span class="size-1.5 rounded-full bg-accent"></span>
      </span>
      <span class="capitalize">{theme}</span>
    </button>
  </li>
{/each}

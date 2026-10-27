<!-- A dropdown of every daisyUI theme, plus "System" to follow the OS light
     or dark setting. A choice dispatches the `phx:set-theme` event the theme
     script in each page's <head> listens for (src/lib/theme_script.js). -->
<script>
  import Icon from "./Icon.svelte";

  const THEMES = [
    "light", "dark", "abyss", "acid", "aqua", "autumn", "black", "bumblebee",
    "business", "caramellatte", "cmyk", "coffee", "corporate", "cupcake",
    "cyberpunk", "dim", "dracula", "emerald", "fantasy", "forest", "garden",
    "halloween", "lemonade", "lofi", "luxury", "night", "nord", "pastel",
    "retro", "silk", "sunset", "synthwave", "valentine", "winter", "wireframe",
  ];

  let open = $state(false);
  let element;
  let current = $state(currentTheme());

  function currentTheme() {
    const root = document.documentElement;
    return root.getAttribute("data-theme-source") === "system" ? "system" : root.getAttribute("data-theme") ?? "system";
  }

  function setTheme(event) {
    event.currentTarget.dispatchEvent(new CustomEvent("phx:set-theme", { bubbles: true, cancelable: true }));
    current = currentTheme();
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

<details class="dropdown dropdown-end" bind:open bind:this={element}>
  <summary class="btn btn-ghost btn-sm rounded-full" title="Theme">
    <Icon name="hero-swatch-micro" class="size-4" />
    <span class="capitalize">{current}</span>
    <Icon name="hero-chevron-down-micro" class="size-4" />
  </summary>
  <ul class="dropdown-content menu z-10 mt-1 max-h-[70vh] w-52 flex-nowrap overflow-y-auto rounded-box bg-base-100 shadow-sm">
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
  </ul>
</details>

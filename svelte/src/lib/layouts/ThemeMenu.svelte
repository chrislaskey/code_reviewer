<!-- A dropdown of the theme choices (ThemeChoices) behind a tile at the
     bottom of the navigation gutter (AppNav); the list opens up and to
     the right of it. -->
<script>
  import { t } from "ttag";
  import Icon from "../components/Icon.svelte";
  import ThemeChoices from "./ThemeChoices.svelte";
  import { currentTheme } from "./themes.js";

  let open = $state(false);
  let element;
  let current = $state(currentTheme());

  function closeWhenClickedOutside(event) {
    if (open && element && !element.contains(event.target)) open = false;
  }

  function closeOnEscape(event) {
    if (event.key === "Escape") open = false;
  }

  $effect(() => {
    const showCurrent = () => (current = currentTheme());
    window.addEventListener("app:theme-changed", showCurrent);
    return () => window.removeEventListener("app:theme-changed", showCurrent);
  });
</script>

<svelte:window onclick={closeWhenClickedOutside} onkeydown={closeOnEscape} />

<details class="dropdown dropdown-right dropdown-end block" bind:open bind:this={element}>
  <summary class="tile" title={t`Theme: ${current}`}>
    <Icon name="hero-swatch" class="tile-icon" />
    {t`Theme`}
  </summary>
  <ul class="dropdown-content menu z-10 ml-1 max-h-[70vh] w-52 flex-nowrap overflow-y-auto rounded-box bg-base-100 shadow-sm">
    <ThemeChoices bind:current onchoose={() => (open = false)} />
  </ul>
</details>

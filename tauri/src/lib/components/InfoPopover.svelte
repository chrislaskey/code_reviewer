<!-- A details element, since WebKit doesn't focus a clicked button. -->
<script>
  import Icon from "./Icon.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} title
   * @property {string} [icon]
   * @property {import("svelte").Snippet<[]>} children
   */

  /** @type {Props} */
  let { title, icon = "hero-information-circle", children } = $props();

  let open = $state(false);
  let element;

  function closeWhenClickedOutside(event) {
    if (open && element && !element.contains(event.target)) open = false;
  }

  function closeOnEscape(event) {
    if (event.key === "Escape") open = false;
  }
</script>

<svelte:window onclick={closeWhenClickedOutside} onkeydown={closeOnEscape} />

<details class="dropdown info-popover" bind:open bind:this={element}>
  <summary class="flex cursor-pointer list-none text-base-content/50 hover:text-base-content" {title}>
    <Icon name={icon} class="size-[1.1rem]" />
  </summary>
  <div class="dropdown-content z-10 mt-1 w-[30rem] rounded-box bg-base-100 p-3 text-xs leading-relaxed text-base-content/70 shadow-sm">
    {@render children()}
  </div>
</details>

<style>
  summary::-webkit-details-marker {
    display: none;
  }
</style>

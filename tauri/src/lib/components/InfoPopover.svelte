<!-- A details element, since WebKit doesn't focus a clicked button. -->
<script>
  import Icon from "./Icon.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} title
   * @property {string} [icon]
   * @property {any} [class] more classes, like daisyUI's `dropdown-top dropdown-end` to open up and to the left
   * @property {import("svelte").Snippet<[]>} children
   */

  /** @type {Props} */
  let { title, icon = "hero-information-circle", class: class_ = "", children } = $props();

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

<details class={["dropdown info-popover", class_]} bind:open bind:this={element}>
  <summary class="flex cursor-pointer list-none text-base-content/50 hover:text-base-content" {title}>
    <Icon name={icon} class="size-[1.1rem]" />
  </summary>
  <div class="dropdown-content panel z-10 mt-1 w-[30rem] p-4 text-sm leading-relaxed text-base-content/70">
    {@render children()}
  </div>
</details>

<style>
  summary::-webkit-details-marker {
    display: none;
  }
</style>

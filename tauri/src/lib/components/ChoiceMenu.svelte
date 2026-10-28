<!-- A button with a caret that drops down a menu of choices (MenuChoice),
     each a title with a sentence saying what it does. The menu closes on a
     click outside it and on Escape; `children` gets `close` to call after a
     choice. A details element rather than daisyUI's focus-based dropdown,
     since WebKit doesn't focus a clicked button. -->
<script>
  import Icon from "./Icon.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} label
   * @property {string} [title]
   * @property {any} [class]
   * @property {import("svelte").Snippet<[{ close: () => void }]>} children
   */

  /** @type {Props} */
  let { label, title, class: class_ = "btn btn-ghost btn-sm", children } = $props();

  let open = $state(false);
  let element;

  function close() {
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
  <summary class={class_} {title}>
    {label}
    <Icon name="hero-chevron-down-micro" class="size-4" />
  </summary>
  <ul class="dropdown-content menu choice-menu w-[22rem] bg-base-100 rounded-box shadow-sm">
    {@render children({ close })}
  </ul>
</details>

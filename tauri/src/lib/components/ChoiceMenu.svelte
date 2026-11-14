<!-- A button with a caret that drops down a menu of choices (MenuChoice),
     each a title with a sentence saying what it does. The menu closes on a
     click outside it and on Escape; `children` gets `close` to call after a
     choice. A details element rather than daisyUI's focus-based dropdown,
     since WebKit doesn't focus a clicked button. On small windows the menu
     spans the window between the page's margins, just below the button,
     since it is wider than a phone; it is fixed there, so its top follows the
     button as the page scrolls. -->
<script>
  import Icon from "./Icon.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} [label]
   * @property {import("svelte").Snippet<[]>} [buttonContent] shown in the button in place of `label`
   * @property {string} [title]
   * @property {any} [class]
   * @property {"start" | "end"} [align] which edge of the button the menu lines up with
   * @property {import("svelte").Snippet<[{ close: () => void }]>} children
   */

  /** @type {Props} */
  let { label, buttonContent, title, class: class_ = "btn btn-ghost", align = "end", children } = $props();

  let open = $state(false);
  let element;
  /** @type {HTMLElement | undefined} */
  let button = $state();
  let menuTop = $state(0);

  $effect(() => {
    if (open) placeMenuBelowButton();
  });

  function placeMenuBelowButton() {
    if (open && button) menuTop = button.getBoundingClientRect().bottom;
  }

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

<svelte:window
  onclick={closeWhenClickedOutside}
  onkeydown={closeOnEscape}
  onscrollcapture={placeMenuBelowButton}
  onresize={placeMenuBelowButton}
/>

<details class={["dropdown", align === "start" ? "dropdown-start" : "dropdown-end"]} bind:open bind:this={element}>
  <summary class={class_} {title} bind:this={button}>
    {#if buttonContent}{@render buttonContent()}{:else}{label}{/if}
    <Icon name="hero-chevron-down-micro" class="size-4" />
  </summary>
  <ul
    class="dropdown-content menu choice-menu panel w-[24rem] max-sm:fixed! max-sm:inset-x-(--page-gutter) max-sm:top-(--menu-top) max-sm:w-auto"
    style:--menu-top="{menuTop}px"
  >
    {@render children({ close })}
  </ul>
</details>

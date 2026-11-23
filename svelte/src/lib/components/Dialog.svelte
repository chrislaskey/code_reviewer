<!-- A dialog over the page, opened with `element.showModal()`; its buttons
     go in a DialogActions at the end. With `closeOnBackdrop`, a click
     beside it closes it. -->
<script>
  import { t } from "ttag";

  /**
   * @typedef {Object} Props
   * @property {HTMLDialogElement} [element]
   * @property {(event: Event) => void} [onclose]
   * @property {(event: Event) => void} [oncancel] on Escape, before it closes
   * @property {boolean} [closeOnBackdrop]
   * @property {any} [class] of the box
   * @property {import("svelte").Snippet} children
   */

  /** @type {Props} */
  let { element = $bindable(), onclose, oncancel, closeOnBackdrop = false, class: class_, children } = $props();
</script>

<dialog bind:this={element} class="modal" {onclose} {oncancel}>
  <div class={["modal-box flex flex-col gap-3", class_]}>
    {@render children()}
  </div>
  {#if closeOnBackdrop}
    <form method="dialog" class="modal-backdrop">
      <button type="submit">{t`Close`}</button>
    </form>
  {/if}
</dialog>

<!-- An emailed code typed one digit to a box: typing moves to the next
     box, Backspace to the one before, and a code pasted or filled in by
     the browser fills every box at once. `value` is the digits so far,
     with no gaps; `oncomplete` is called once every box is filled. The
     first empty box takes the focus when it appears and from `focus()`. -->
<script>
  import { onMount } from "svelte";

  /**
   * @typedef {Object} Props
   * @property {string} [value]
   * @property {number} [length]
   * @property {boolean} [disabled]
   * @property {string} [label] for screen readers
   * @property {(code: string) => void} [oncomplete]
   */

  /** @type {Props} */
  let { value = $bindable(""), length = 6, disabled = false, label = "Code", oncomplete } = $props();

  /** @type {HTMLInputElement[]} */
  let boxes = $state([]);
  const indexes = $derived(Array.from({ length }, (_, index) => index));

  onMount(() => focus());

  /** Focuses the first empty box, or the last when all are filled. */
  export function focus() {
    boxes[Math.min(value.length, length - 1)]?.focus();
  }

  /** @param {string} text */
  function digitsOf(text) {
    return text.replace(/\D/g, "");
  }

  /** @param {number} index @param {string} digits */
  function fillFrom(index, digits) {
    value = (value.slice(0, index) + digits + value.slice(index + digits.length)).slice(0, length);
    boxes[Math.min(index + digits.length, length - 1)]?.focus();
    if (value.length === length) oncomplete?.(value);
  }

  /** @param {number} index */
  function removeAt(index) {
    value = value.slice(0, index) + value.slice(index + 1);
  }

  /** @param {Event & {currentTarget: HTMLInputElement}} event @param {number} index */
  function input(event, index) {
    const box = event.currentTarget;
    const before = value[index] ?? "";
    let digits = digitsOf(box.value);
    // One digit typed after the one the box held
    if (before && digits.length === 2 && digits.startsWith(before)) digits = digits.slice(1);
    if (digits) fillFrom(index, digits);
    else removeAt(index);
    box.value = value[index] ?? "";
  }

  /** @param {KeyboardEvent} event @param {number} index */
  function keydown(event, index) {
    if (event.key === "Backspace") {
      event.preventDefault();
      if (value[index]) removeAt(index);
      else if (index > 0) {
        removeAt(index - 1);
        boxes[index - 1]?.focus();
      }
    } else if (event.key === "ArrowLeft" && index > 0) {
      event.preventDefault();
      boxes[index - 1]?.focus();
    } else if (event.key === "ArrowRight" && index < value.length) {
      event.preventDefault();
      boxes[index + 1]?.focus();
    }
  }

  /** @param {ClipboardEvent} event @param {number} index */
  function paste(event, index) {
    event.preventDefault();
    const digits = digitsOf(event.clipboardData?.getData("text") ?? "");
    if (digits) fillFrom(index, digits);
  }

  /**
   * No gaps: a box past the first empty one hands the focus to it.
   * @param {FocusEvent & {currentTarget: HTMLInputElement}} event @param {number} index
   */
  function focusin(event, index) {
    if (index > value.length) boxes[value.length]?.focus();
    else event.currentTarget.select();
  }
</script>

<div role="group" aria-label={label} class="flex gap-2">
  {#each indexes as index (index)}
    <input
      bind:this={boxes[index]}
      value={value[index] ?? ""}
      oninput={(event) => input(event, index)}
      onkeydown={(event) => keydown(event, index)}
      onpaste={(event) => paste(event, index)}
      onfocus={(event) => focusin(event, index)}
      {disabled}
      inputmode="numeric"
      autocomplete={index === 0 ? "one-time-code" : "off"}
      aria-label={`${label}, digit ${index + 1} of ${length}`}
      class="input h-12 w-11 px-0 text-center font-mono text-xl"
    />
  {/each}
</div>

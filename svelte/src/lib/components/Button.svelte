<!-- A button, or a link drawn as one when it has an `href`: daisyUI's btn
     in one of the app's variants and sizes. Other attributes, such as
     `disabled`, `onclick` and `target`, pass through. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {"default" | "primary" | "ghost" | "danger"} [variant]
   * @property {"md" | "sm" | "xs"} [size]
   * @property {string} [href] makes it a link
   * @property {"button" | "submit"} [type]
   * @property {any} [class]
   * @property {import("svelte").Snippet} children
   */

  /** @type {Props & Record<string, any>} */
  let { variant = "default", size = "md", href, type = "button", class: class_, children, ...attributes } = $props();

  const VARIANT_CLASSES = { default: "", primary: "btn-primary", ghost: "btn-ghost", danger: "btn-error" };
  const SIZE_CLASSES = { md: "", sm: "btn-sm", xs: "btn-xs" };

  let classes = $derived(["btn", VARIANT_CLASSES[variant], SIZE_CLASSES[size], class_]);
</script>

{#if href}
  <a {href} class={classes} {...attributes}>{@render children()}</a>
{:else}
  <button {type} class={classes} {...attributes}>{@render children()}</button>
{/if}

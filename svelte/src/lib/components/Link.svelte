<!-- A link in running text, in one of the app's tones and sizes; without
     an `href`, a button drawn as a link, for an action such as "Send a new
     code". Other attributes, such as `target`, `onclick` and `disabled`,
     pass through. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} [href] a link; without one, a button
   * @property {"default" | "danger" | "warning"} [tone]
   * @property {"md" | "sm"} [size]
   * @property {any} [class]
   * @property {import("svelte").Snippet} children
   */

  /** @type {Props & Record<string, any>} */
  let { href, tone = "default", size = "md", class: class_, children, ...attributes } = $props();

  const TONE_CLASSES = { default: "", danger: "text-error", warning: "text-warning" };

  let classes = $derived(["link", TONE_CLASSES[tone], size === "sm" && "text-sm", class_]);
</script>

{#if href !== undefined}
  <a {href} class={classes} {...attributes}>{@render children()}</a>
{:else}
  <button type="button" class={classes} {...attributes}>{@render children()}</button>
{/if}

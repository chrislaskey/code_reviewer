<!-- Running text: a paragraph by default, a span inside other text, or a
     fieldset's legend, in one of the app's tones and sizes. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {"default" | "muted" | "error" | "warning"} [tone]
   * @property {"md" | "sm"} [size]
   * @property {"p" | "span" | "legend"} [as]
   * @property {any} [class]
   * @property {import("svelte").Snippet} children
   */

  /** @type {Props & Record<string, any>} */
  let { tone = "default", size = "md", as = "p", class: class_, children, ...attributes } = $props();

  const TONE_CLASSES = { default: "", muted: "muted", error: "error", warning: "text-warning" };
</script>

<svelte:element this={as} class={[TONE_CLASSES[tone], size === "sm" && "text-sm", class_]} {...attributes}>
  {@render children()}
</svelte:element>

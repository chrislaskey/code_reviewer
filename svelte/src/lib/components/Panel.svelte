<!-- A raised box that sets one thing apart, such as a post in a list:
     its parts stacked, or in a `row`; `as="li"` in a list. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {"div" | "li" | "article" | "section"} [as]
   * @property {"stack" | "row"} [layout]
   * @property {any} [class]
   * @property {import("svelte").Snippet} children
   */

  /** @type {Props & Record<string, any>} */
  let { as = "div", layout = "stack", class: class_, children, ...attributes } = $props();
</script>

<svelte:element this={as} class={["panel", `panel-${layout}`, class_]} {...attributes}>
  {@render children()}
</svelte:element>

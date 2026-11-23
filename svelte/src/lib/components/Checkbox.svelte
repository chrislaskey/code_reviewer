<!-- A checkbox, or a switch with `toggle`, with its label beside it. Other
     attributes pass through to the <input>. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {boolean} [checked]
   * @property {boolean} [toggle] draws it as a switch
   * @property {"md" | "sm"} [size] of the label's text
   * @property {"default" | "muted"} [tone] of the label's text
   * @property {any} [class]
   * @property {import("svelte").Snippet} children the label
   */

  /** @type {Props & Record<string, any>} */
  let { checked = $bindable(false), toggle = false, size = "md", tone = "default", class: class_, children, ...attributes } = $props();
</script>

<label class={["flex items-center gap-2", size === "sm" && "text-sm", tone === "muted" && "muted", class_]}>
  <input bind:checked type="checkbox" class={toggle ? "toggle toggle-sm" : "checkbox checkbox-sm"} {...attributes} />
  {@render children()}
</label>

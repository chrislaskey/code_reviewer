<!-- A box that stands out from the page, such as a banner: daisyUI's alert
     in one of the app's tones. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {"info" | "warning" | "error"} [tone]
   * @property {any} [class]
   * @property {import("svelte").Snippet} children
   */

  /** @type {Props & Record<string, any>} */
  let { tone = "info", class: class_, children, ...attributes } = $props();

  const TONE_CLASSES = { info: "alert-info", warning: "alert-warning", error: "alert-error" };
</script>

<div class={["alert", TONE_CLASSES[tone], class_]} {...attributes}>
  {@render children()}
</div>

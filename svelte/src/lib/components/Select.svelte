<!-- A choice of one among a few, the width of its form: its options are
     the <option>s inside. Other attributes, such as `onchange` and
     `disabled`, pass through. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} [value]
   * @property {any} [class]
   * @property {import("svelte").Snippet} children the options
   */

  /** @type {Props & Record<string, any>} */
  let { value = $bindable(""), class: class_, children, ...attributes } = $props();
</script>

<select bind:value class={["select w-full", class_]} {...attributes}>
  {@render children()}
</select>

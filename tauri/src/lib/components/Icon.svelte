<!-- A heroicon by its `hero-*` class, see vendor/heroicons.js. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {any} [class]
   * @property {string} name
   */

  /** @type {Props} */
  let { class: class_ = "size-4", name } = $props();
</script>

<span class={[name, class_]}></span>

<!-- A heroicon by its `hero-*` class, or a Lucide icon by its `lucide-*`
     class, see vendor/heroicons.js and vendor/lucide.js. -->
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

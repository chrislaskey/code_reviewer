<!-- A one-line text field the width of its form: text, email, password,
     search and the like. Other attributes pass through; `element` binds
     the <input>, to focus it. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} [value]
   * @property {string} [type]
   * @property {HTMLInputElement} [element]
   * @property {any} [class]
   */

  /** @type {Props & Record<string, any>} */
  let { value = $bindable(""), type = "text", element = $bindable(), class: class_, ...attributes } = $props();
</script>

<input bind:this={element} bind:value {type} class={["input w-full", class_]} {...attributes} />

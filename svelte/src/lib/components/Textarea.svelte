<!-- A text field of several lines, the width of its form. Other attributes
     pass through. -->
<script>
  /** @type {{ value?: string, class?: any } & Record<string, any>} */
  let { value = $bindable(""), class: class_, ...attributes } = $props();
</script>

<textarea bind:value class={["textarea w-full", class_]} {...attributes}></textarea>

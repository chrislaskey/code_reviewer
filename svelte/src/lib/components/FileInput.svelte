<!-- A file picker the width of its form; the page reads the file from its
     `onchange`. Other attributes pass through. -->
<script>
  /** @type {{ element?: HTMLInputElement, class?: any } & Record<string, any>} */
  let { element = $bindable(), class: class_, ...attributes } = $props();
</script>

<input bind:this={element} type="file" class={["file-input w-full", class_]} {...attributes} />

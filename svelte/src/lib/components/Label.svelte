<!-- A form control's label: its `text` above the control and anything
     under it, such as the control's errors, all inside the <label> so a
     click on the text focuses the control. -->
<script>
  /** @type {{ text: string, class?: any, children: import("svelte").Snippet }} */
  let { text, class: class_, children } = $props();
</script>

<label class={["field", class_]}>
  <span>{text}</span>
  {@render children()}
</label>

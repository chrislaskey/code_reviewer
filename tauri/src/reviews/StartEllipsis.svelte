<!-- Long text clipped at the start, so the end of a path stays visible. -->
<script>
  /** @type {{ text: string, class?: any, title?: string }} */
  let { text, class: class_ = "", title } = $props();
</script>

<p dir="rtl" title={title ?? text} class={["truncate text-left", class_]}>
  <bdi>{text}</bdi>
</p>

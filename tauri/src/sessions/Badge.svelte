<script>
  let { text, kind = "badge-outline text-base-content/60", title = "" } = $props();
</script>

<span class="badge badge-sm whitespace-nowrap {kind}" {title}>{text}</span>

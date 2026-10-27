<script>
  let { text, kind = "", title = "" } = $props();
</script>

<span class="pill {kind}" {title}>{text}</span>

<!-- Shows the end of a long path, where the interesting part is, with ~ for
     the home directory. -->
<script>
  let { path, homeDirectory } = $props();

  let short = $derived(
    homeDirectory && path.startsWith(homeDirectory) ? `~${path.slice(homeDirectory.length)}` : path,
  );
</script>

<div class="path" title={path}><span dir="ltr">{short}</span></div>

<script>
  let { path, git } = $props();

  let name = $derived(git?.repository_name ?? path.split("/").filter(Boolean).pop() ?? path);
</script>

<span class="repository-name" title={path}>{name}</span>

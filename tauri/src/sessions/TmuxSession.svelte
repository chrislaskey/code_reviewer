<script>
  import SessionStatusBar from "./SessionStatusBar.svelte";

  let { session, mainBranch, actions } = $props();

  let working = $derived(session.agent?.status === "busy");
</script>

<div class="panel" class:working>
  <SessionStatusBar {session} {mainBranch} {actions} />
</div>

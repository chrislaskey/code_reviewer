<script>
  import Pill from "./Pill.svelte";

  const PILL_KINDS = { idle: "pill-good", busy: "pill-busy" };

  let { session } = $props();

  let status = $derived(session.status ?? "unknown");
</script>

<Pill text="claude {status}" kind={PILL_KINDS[status] ?? ""} title="Claude Code reports “{status}”" />

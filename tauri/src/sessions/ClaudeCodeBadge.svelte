<script>
  import Badge from "./Badge.svelte";
  import { timeAgo } from "./formatting.js";

  const BADGE_TEXTS = { busy: "working" };

  let { session } = $props();

  let status = $derived(session.status ?? "unknown");
  let title = $derived(
    `Claude Code reports “${status}”` +
      (session.status_updated_at ? `, last changed ${timeAgo(session.status_updated_at)}` : ""),
  );
</script>

<Badge text={BADGE_TEXTS[status] ?? status} kind="badge-ghost" {title} />

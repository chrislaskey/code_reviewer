<script>
  import Badge from "./Badge.svelte";
  import { timeAgo } from "./formatting.js";

  const BADGE_TEXTS = { busy: "working" };

  let { session } = $props();

  let status = $derived(session.status ?? "unknown");
  let verb = $derived(session.kind === "cursor_agent" ? "looks" : "reports");
  let title = $derived(
    `The agent ${verb} “${status}”` +
      (session.kind === "cursor_agent" ? ", judged by its recent writes to its chat" : "") +
      (session.status_updated_at ? `, last changed ${timeAgo(session.status_updated_at)}` : ""),
  );
</script>

<Badge text={BADGE_TEXTS[status] ?? status} kind="badge-ghost" {title} />

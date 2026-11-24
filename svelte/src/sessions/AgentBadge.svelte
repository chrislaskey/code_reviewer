<script>
  import Badge from "./Badge.svelte";
  import { timeAgo } from "./formatting.js";

  const BADGE_TEXTS = { busy: "working" };
  // These agents write no status, so it's guessed from their writes to their chat.
  const GUESSED_STATUS_KINDS = ["cursor_agent", "codex"];

  let { session } = $props();

  let status = $derived(session.status ?? "unknown");
  let isGuessed = $derived(GUESSED_STATUS_KINDS.includes(session.kind));
  let title = $derived(
    `The agent ${isGuessed ? "looks" : "reports"} “${status}”` +
      (isGuessed ? ", judged by its recent writes to its chat" : "") +
      (session.status_updated_at ? `, last changed ${timeAgo(session.status_updated_at)}` : ""),
  );
</script>

<Badge text={BADGE_TEXTS[status] ?? status} kind="badge-ghost" {title} />

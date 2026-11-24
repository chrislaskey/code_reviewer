<script>
  import { t } from "ttag";
  import Badge from "./Badge.svelte";
  import { timeAgo } from "./formatting.js";

  const BADGE_TEXTS = { busy: t`working`, idle: t`idle`, unknown: t`unknown` };
  // These agents write no status, so it's guessed from their writes to their chat.
  const GUESSED_STATUS_KINDS = ["cursor_agent", "codex"];

  let { session } = $props();

  let status = $derived(session.status ?? "unknown");
  let isGuessed = $derived(GUESSED_STATUS_KINDS.includes(session.kind));
  let statusText = $derived(BADGE_TEXTS[status] ?? status);
  let title = $derived.by(() => {
    const reported = isGuessed
      ? t`The agent looks “${statusText}”, judged by its recent writes to its chat`
      : t`The agent reports “${statusText}”`;
    if (!session.status_updated_at) return reported;
    const changedAgo = timeAgo(session.status_updated_at);
    return t`${reported}, last changed ${changedAgo}`;
  });
</script>

<Badge text={statusText} kind="badge-ghost" {title} />

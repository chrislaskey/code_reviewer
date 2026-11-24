<!-- Marks a session card whose bell is on for this device (see NotificationBell.svelte). Read only: the bell is
     turned on and off on the session's page. -->
<script>
  import Icon from "../lib/components/Icon.svelte";

  const TITLE = "Notifying when the agent finishes";
</script>

<span class="notifications-enabled-icon" title={TITLE} role="img" aria-label={TITLE}>
  <Icon name="lucide-bell-ring" class="size-4" />
</span>

<style>
  .notifications-enabled-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--color-primary);
  }
</style>

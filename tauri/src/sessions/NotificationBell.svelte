<!-- A session's bell, at the top of its page: while it's on, this device gets a system notification each time the
     session's agent finishes. Each device has its own; a phone's comes as a push, so turning it on there asks for
     permission the first time. -->
<script>
  import { onMount } from "svelte";
  import Icon from "../lib/components/Icon.svelte";
  import { getSessionNotificationsEnabled, setSessionNotificationsEnabled } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";

  /**
   * @typedef {Object} Props
   * @property {string} name
   */

  /** @type {Props} */
  let { name } = $props();

  let shown = $state(false);
  let enabled = $state(false);
  let saving = $state(false);
  let title = $derived(
    enabled ? "Notifying when the agent finishes. Turn off" : "Notify me each time the agent finishes",
  );

  async function toggle() {
    saving = true;
    try {
      enabled = await setSessionNotificationsEnabled(name, !enabled);
      log(`${name}: ${enabled ? "Notifying when the agent finishes" : "Stopped notifying"}`);
    } catch (error) {
      log(`Changing the notifications for ${name} failed: ${error}`);
    }
    saving = false;
  }

  onMount(async () => {
    try {
      enabled = await getSessionNotificationsEnabled(name);
      shown = true;
    } catch (error) {
      log(`Reading the notifications for ${name} failed: ${error}`);
    }
  });
</script>

{#if shown}
  <button
    type="button"
    class={["btn text-base btn-primary", !enabled && "btn-soft"]}
    {title}
    aria-label={title}
    aria-pressed={enabled}
    disabled={saving}
    onclick={toggle}
  >
    <Icon name={enabled ? "lucide-bell-ring" : "lucide-bell"} class="size-5" />
  </button>
{/if}

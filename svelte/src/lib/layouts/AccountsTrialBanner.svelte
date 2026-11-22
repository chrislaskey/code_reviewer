<!-- Shown on every page to a trial user, with no way to close it: how
     long the trial has left, then that it ended and until when signing
     up still keeps what they made, see
     docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { accountsSignInUrl } from "../routes.js";
  import { Alert, Button } from "../components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} trialExpiresAt when the 24 hours end
   */

  /** @type {Props} */
  let { trialExpiresAt } = $props();

  /** How long after the trial ends the server keeps what was made. */
  const KEPT_AFTER_HOURS = 24;
  const HOUR = 60 * 60 * 1000;

  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 60 * 1000);
    return () => clearInterval(timer);
  });

  let endsAt = $derived(new Date(trialExpiresAt).getTime());
  let left = $derived(endsAt - now);
  let deletedAt = $derived(new Date(endsAt + KEPT_AFTER_HOURS * HOUR));

  /** @param {number} milliseconds */
  function timeLeft(milliseconds) {
    const hours = Math.floor(milliseconds / HOUR);
    if (hours >= 1) return `${hours} ${hours === 1 ? "hour" : "hours"}`;
    const minutes = Math.max(1, Math.ceil(milliseconds / (60 * 1000)));
    return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
  }
</script>

<Alert tone={left > 0 ? "warning" : "error"} class="flex flex-wrap items-center gap-3" role="status" data-trial-banner>
  {#if left > 0}
    <span class="grow">This is a trial. Sign up within {timeLeft(left)} to keep what you make.</span>
  {:else}
    <span class="grow">
      Your trial ended. Sign up before {deletedAt.toLocaleString([], { dateStyle: "medium", timeStyle: "short" })} to keep
      what you made.
    </span>
  {/if}
  <Button size="sm" href={accountsSignInUrl("")}>Sign up</Button>
</Alert>

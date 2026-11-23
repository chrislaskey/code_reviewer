<!-- What being a guest means, for a trial user: how long is left to sign
     up and keep what they make, then that it ended and until when signing
     up still keeps it, with Sign up. In the account menu, which a dot on
     the avatar points to, and on the profile page; the rest of the app
     stays as it is. See docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { t } from "ttag";
  import { accountsSignInUrl } from "./routes.js";
  import { Button, Text } from "./components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} trialExpiresAt when the 24 hours end
   * @property {() => void} [onsignup] the menu closes on Sign up
   */

  /** @type {Props} */
  let { trialExpiresAt, onsignup } = $props();

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
  let remaining = $derived(timeLeft(left));
  let deletedAt = $derived(
    new Date(endsAt + KEPT_AFTER_HOURS * HOUR).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }),
  );

  /** @param {number} milliseconds */
  function timeLeft(milliseconds) {
    const hours = Math.floor(milliseconds / HOUR);
    if (hours >= 1) return hours === 1 ? t`1 hour` : t`${hours} hours`;
    const minutes = Math.max(1, Math.ceil(milliseconds / (60 * 1000)));
    return minutes === 1 ? t`1 minute` : t`${minutes} minutes`;
  }
</script>

<div class="flex flex-col items-start gap-2" role="status" data-guest-notice>
  {#if left > 0}
    <Text size="sm">{t`You're a guest. Sign up within ${remaining} to keep what you make.`}</Text>
  {:else}
    <Text size="sm" tone="error">{t`Your guest visit ended. Sign up before ${deletedAt} to keep what you made.`}</Text>
  {/if}
  <Button variant="primary" size="sm" href={accountsSignInUrl("")} onclick={onsignup}>{t`Sign up`}</Button>
</div>

<!-- The second step of a sign-up or a sign-in: the six-digit code that
     was emailed, and the error the attempt came back with. -->
<script>
  import { Button, ErrorMessage, Input, Link, Text } from "../lib/components/index.js";
  /**
   * @typedef {Object} Props
   * @property {string} email where the code went
   * @property {string} code
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   * @property {() => void} onresend a new code to the same email
   * @property {boolean} [resent] a new code was just sent
   * @property {() => void} onstartover back to the email and password
   */

  /** @type {Props} */
  let { email, code = $bindable(""), error = "", busy = false, onsubmit, onresend, resent = false, onstartover } = $props();

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <Text>We sent a code to <strong>{email}</strong>. Enter it to finish; it expires in ten minutes.</Text>
  <Input
    bind:value={code}
    inputmode="numeric"
    autocomplete="one-time-code"
    pattern="[0-9 ]*"
    aria-label="Code"
    placeholder="Code"
    required
  />
  <ErrorMessage message={error} />
  {#if resent}
    <Text tone="muted" size="sm">A new code is on its way; the old one no longer works.</Text>
  {/if}
  <div class="flex flex-wrap items-center gap-3">
    <Button variant="primary" type="submit" disabled={busy}>Continue</Button>
    <Link size="sm" onclick={onresend} disabled={busy}>Send a new code</Link>
    <Link size="sm" onclick={onstartover} disabled={busy}>Start over</Link>
  </div>
</form>

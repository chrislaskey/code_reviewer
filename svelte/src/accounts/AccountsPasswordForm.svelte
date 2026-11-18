<!-- The step after the code, when there is one: the account's password,
     a new one in place of a forgotten one, or, at a first sign-in, a
     password to make or skip. -->
<script>
  import { Button, ErrorMessage, Heading, Input, Link, Text } from "../lib/components/index.js";
  /**
   * @typedef {Object} Props
   * @property {boolean} isNew a password to make, which may be skipped
   * @property {boolean} [resetting] a new password in place of a forgotten one
   * @property {() => void} [onforgot] given when a forgotten password may be reset
   * @property {string} password
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   * @property {() => void} [onskip] for a new password
   * @property {() => void} onstartover back to the email
   */

  /** @type {Props} */
  let { isNew, resetting = false, password = $bindable(""), error = "", busy = false, onsubmit, onskip, onforgot, onstartover } = $props();

  /** A password being made, not checked. */
  let making = $derived(isNew || resetting);

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  {#if isNew}
    <Heading>Create a password</Heading>
    <Text>A password is optional. With one, signing in asks for it after the emailed code. You can make one later from your account.</Text>
  {:else if resetting}
    <Heading>Reset your password</Heading>
    <Text>The code proved this is your email. Choose a new password to finish signing in; the old one stops working.</Text>
  {:else}
    <Text>Enter your password to finish.</Text>
  {/if}
  <Input
    bind:value={password}
    type="password"
    autocomplete={making ? "new-password" : "current-password"}
    minlength={making ? 12 : undefined}
    aria-label="Password"
    placeholder={making ? "Password, 12 characters or more" : "Password"}
    required
  />
  <ErrorMessage message={error} />
  <div class="flex flex-wrap items-center gap-3">
    <Button variant="primary" type="submit" disabled={busy}>{isNew ? "Create password" : resetting ? "Reset password" : "Continue"}</Button>
    {#if isNew}
      <Button onclick={onskip} disabled={busy} data-skip>Skip</Button>
    {:else}
      {#if onforgot && !resetting}
        <Link size="sm" onclick={onforgot} disabled={busy}>Forgot your password?</Link>
      {/if}
      <Link size="sm" onclick={onstartover} disabled={busy}>Start over</Link>
    {/if}
  </div>
</form>

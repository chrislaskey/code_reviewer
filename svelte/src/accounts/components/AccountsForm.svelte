<!-- The email field, for the Sign in page, with the choice to stay signed in on this browser, and
     the error the attempt came back with. -->
<script>
  import { Button, Checkbox, ErrorMessage, Input, Text } from "../../lib/components/index.js";
  /**
   * @typedef {Object} Props
   * @property {string} email
   * @property {boolean} [keepSignedIn] bound where the choice is offered
   * @property {boolean} [withKeepSignedIn] not for a device's sign-in, which leaves nothing in the browser
   * @property {string} submitText
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   */

  /** @type {Props} */
  let { email = $bindable(""), keepSignedIn = $bindable(false), withKeepSignedIn = false, submitText, error = "", busy = false, onsubmit } = $props();

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <Input bind:value={email} type="email" autocomplete="email" aria-label="Email" placeholder="Email" required />
  {#if withKeepSignedIn}
    <Checkbox bind:checked={keepSignedIn} size="sm">Keep me signed in on this browser</Checkbox>
  {/if}
  <ErrorMessage message={error} />
  <Text tone="muted" size="sm">We will email you a code to continue.</Text>
  <div>
    <Button variant="primary" type="submit" disabled={busy}>{submitText}</Button>
  </div>
</form>

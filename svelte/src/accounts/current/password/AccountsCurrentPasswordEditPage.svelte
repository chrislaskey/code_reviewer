<!-- The logged-in person's password, at /accounts/current/password/edit:
     their first, or a new one, no current one asked. A password is a way
     to log in beside the emailed code; a forgotten one is changed here
     after logging in with a code, and the account's email is told. The
     server keeps it, so in the app this needs a connection. -->
<script>
  import { accountsPasswordGet, accountsPasswordUpdate } from "../../../lib/core/index.js";
  import { errorMessageOf } from "../../../lib/error_message.js";
  import { accountsCurrentShowUrl } from "../../../lib/routes.js";
  import { Button, ErrorMessage, Heading, Input, Link, Page, Text } from "../../../lib/components/index.js";

  /** Whether the account has a password; null until the server answered. */
  let hasPassword = $state(/** @type {boolean | null} */ (null));
  let password = $state("");
  let error = $state("");
  let saved = $state(false);
  let busy = $state(false);

  accountsPasswordGet().then(
    (status) => (hasPassword = status.is_set),
    (problem) => (error = errorMessageOf(problem)),
  );

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    busy = true;
    error = "";
    saved = false;
    try {
      await accountsPasswordUpdate(password);
      hasPassword = true;
      saved = true;
      password = "";
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>{hasPassword ? "Change password" : "Create a password"}</Heading>
  {#if hasPassword !== null}
    <form onsubmit={save} class="flex flex-col gap-3">
      {#if !hasPassword}
        <Text>A password is optional. With one, you can log in with it instead of an emailed code.</Text>
      {/if}
      <Input
        bind:value={password}
        type="password"
        autocomplete="new-password"
        minlength="12"
        aria-label="New password"
        placeholder="New password, 12 characters or more"
        required
      />
      <Text tone="muted" size="sm">We'll email you when your password changes.</Text>
      {#if saved}
        <Text tone="muted" data-saved>Your password is saved.</Text>
      {/if}
      <ErrorMessage message={error} />
      <div class="flex flex-wrap items-center gap-3">
        <Button variant="primary" type="submit" disabled={busy}>{hasPassword ? "Change password" : "Create password"}</Button>
        <Link size="sm" href={accountsCurrentShowUrl()}>Back to account</Link>
      </div>
    </form>
  {:else if error}
    <ErrorMessage message={error} />
  {/if}
</Page>

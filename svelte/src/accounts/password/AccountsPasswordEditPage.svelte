<!-- The signed-in person's password, at /accounts/password/edit: their first,
     or a new one in place of the current one. A password is optional,
     asked for after the emailed code at a sign-in once the account has
     one. The server keeps it, so in the app this needs a connection. -->
<script>
  import { accountsPasswordUpdate, accountsSecondFactors } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { accountsCurrentShowUrl } from "../../lib/routes.js";
  import { Button, ErrorMessage, Heading, Input, Link, Page, Text } from "../../lib/components/index.js";

  /** Whether the account has a password; null until the server answered. */
  let hasPassword = $state(/** @type {boolean | null} */ (null));
  let current = $state("");
  let password = $state("");
  let error = $state("");
  let saved = $state(false);
  let busy = $state(false);

  accountsSecondFactors().then(
    (factors) => (hasPassword = factors.includes("password")),
    (problem) => (error = errorMessageOf(problem)),
  );

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    busy = true;
    error = "";
    saved = false;
    try {
      await accountsPasswordUpdate(hasPassword ? current : null, password);
      hasPassword = true;
      saved = true;
      current = "";
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
      {#if hasPassword}
        <Input bind:value={current} type="password" autocomplete="current-password" aria-label="Current password" placeholder="Current password" required />
        <Text tone="muted" size="sm">Forgot it? Sign out and sign in again; the password step offers a reset when this app allows one.</Text>
      {:else}
        <Text>A password is optional. With one, signing in asks for it after the emailed code.</Text>
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

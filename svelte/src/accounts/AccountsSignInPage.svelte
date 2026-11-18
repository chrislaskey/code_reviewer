<!-- Signing in, at /accounts/sign-in, on the web: the email, then the
     code that was emailed and the password if the account has one, see
     AccountsSignInSteps. Opened by a device with ?challenge=, it ends in
     a code for the device instead, see docs/design/accounts-sign-in.md.
     The email is filled in from ?email=. -->
<script>
  import { accountsSignIn, accountsTrialGet } from "../lib/core/index.js";
  import { accountsSignUpUrl, accountsTrialUrl } from "../lib/routes.js";
  import AccountsForm from "./AccountsForm.svelte";
  import AccountsSignInSteps from "./AccountsSignInSteps.svelte";
  import { KEEP_SIGNED_IN_CHECKED } from "./keep_signed_in.js";
  import { Heading, Link, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";

  let email = $state(query.get("email") ?? "");
  let keepSignedIn = $state(KEEP_SIGNED_IN_CHECKED);
  /** Whether to offer a trial: on the web, when the server has room for one. */
  let trialAvailable = $state(false);
  if (!challenge) accountsTrialGet().then((available) => (trialAvailable = available), () => {});
  let pending = $state("");
  let error = $state("");
  let busy = $state(false);

  async function signIn() {
    busy = true;
    error = "";
    try {
      pending = (await accountsSignIn(email)).pending;
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>Sign in</Heading>
  {#if challenge}
    <Text tone="muted">Signing in for the app on your device.</Text>
  {/if}
  {#if pending}
    <AccountsSignInSteps {email} {pending} {challenge} {keepSignedIn} onstartover={() => (pending = "")} />
  {:else}
    <AccountsForm bind:email bind:keepSignedIn withKeepSignedIn={!challenge} submitText="Sign in" {error} {busy} onsubmit={signIn} />
    <Text tone="muted">New here? <Link href={accountsSignUpUrl(email || undefined)}>Sign up</Link></Text>
    {#if trialAvailable}
      <Text tone="muted">Just looking? <Link href={accountsTrialUrl()}>Try it without an account</Link></Text>
    {/if}
  {/if}
</Page>

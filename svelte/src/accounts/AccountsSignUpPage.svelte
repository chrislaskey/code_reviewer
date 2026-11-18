<!-- Signing up, at /accounts/sign-up, on the web: the name and the
     email, then the code that was emailed, which proves the email, and a
     password to make or skip, see AccountsSignInSteps. Opened by a
     device with ?challenge=, it ends in a code for the device instead;
     the email is filled in from ?email=. -->
<script>
  import PublicPagesLinks from "../lib/PublicPagesLinks.svelte";
  import { accountsSignUp, accountsTrialGet } from "../lib/core/index.js";
  import { accountsSignInUrl, accountsTrialUrl } from "../lib/routes.js";
  import AccountsForm from "./AccountsForm.svelte";
  import AccountsSignInSteps from "./AccountsSignInSteps.svelte";
  import { KEEP_SIGNED_IN_CHECKED } from "./keep_signed_in.js";
  import { Heading, Link, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";

  let email = $state(query.get("email") ?? "");
  let name = $state("");
  let keepSignedIn = $state(KEEP_SIGNED_IN_CHECKED);
  /** Whether to offer a trial: on the web, when the server has room for one. */
  let trialAvailable = $state(false);
  if (!challenge) accountsTrialGet().then((available) => (trialAvailable = available), () => {});
  let pending = $state("");
  let error = $state("");
  let busy = $state(false);

  async function signUp() {
    busy = true;
    error = "";
    try {
      pending = (await accountsSignUp(email, name)).pending;
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>Sign up</Heading>
  {#if challenge}
    <Text tone="muted">Signing up for the app on your device.</Text>
  {/if}
  {#if pending}
    <AccountsSignInSteps {email} {pending} {challenge} {keepSignedIn} onstartover={() => (pending = "")} />
  {:else}
    <AccountsForm bind:email bind:name withName bind:keepSignedIn withKeepSignedIn={!challenge} submitText="Sign up" {error} {busy} onsubmit={signUp} />
    <Text tone="muted">By signing up you accept the <PublicPagesLinks />.</Text>
    <Text tone="muted">Already have an account? <Link href={accountsSignInUrl(email || undefined)}>Sign in</Link></Text>
    {#if trialAvailable}
      <Text tone="muted">Just looking? <Link href={accountsTrialUrl()}>Try it without an account</Link></Text>
    {/if}
  {/if}
</Page>

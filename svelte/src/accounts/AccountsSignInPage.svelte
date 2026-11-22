<!-- Signing in or up, at /accounts/sign-in, on the web, one flow for
     both: the email, then the code that was emailed and the button that
     makes the account or signs in to it, see AccountsSignInSteps; the
     server knows which, and the email step looks the same either way.
     Opened by a device with ?challenge=, it ends in a code for the device
     instead, see docs/design/accounts-sign-in.md. The email is filled in
     from ?email=. A trial is offered where the server has room for one. -->
<script>
  import { accountsSignIn, accountsTrialGet } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentServer, currentServerLoad } from "../lib/current_server.svelte.js";
  import { accountsTrialUrl } from "../lib/routes.js";
  import AccountsForm from "./components/AccountsForm.svelte";
  import AccountsSignInSteps from "./components/AccountsSignInSteps.svelte";
  import { KEEP_SIGNED_IN_CHECKED } from "./keep_signed_in.js";
  import { Heading, Link, Page, Text } from "../lib/components/index.js";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";
  const appReturn = { appPort: query.get("app_port") ?? "", appScheme: query.get("app_scheme") ?? "" };

  let email = $state(query.get("email") ?? "");
  let keepSignedIn = $state(KEEP_SIGNED_IN_CHECKED);
  /** Whether to offer a trial: on the web, when the server has room for one. */
  let trialAvailable = $state(false);
  if (!challenge) accountsTrialGet().then((available) => (trialAvailable = available), () => {});
  currentServerLoad().catch(() => {});
  /** Whether the server takes new sign-ups, as its settings say. */
  let signUpOffered = $derived(currentServer.value?.settings.features.accounts_sign_up ?? true);
  let pending = $state("");
  let error = $state("");
  let busy = $state(false);

  async function signIn() {
    busy = true;
    error = "";
    try {
      pending = (await accountsSignIn(email)).pending;
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>{signUpOffered ? "Sign in or create an account" : "Sign in"}</Heading>
  {#if challenge}
    <Text tone="muted">Signing in for the app on your device.</Text>
  {/if}
  {#if pending}
    <AccountsSignInSteps {email} {pending} {challenge} {appReturn} {keepSignedIn} onstartover={() => (pending = "")} />
  {:else}
    <AccountsForm bind:email bind:keepSignedIn withKeepSignedIn={!challenge} submitText="Continue" {error} {busy} onsubmit={signIn} />
    {#if !signUpOffered}
      <Text tone="muted">This server takes no new sign-ups.</Text>
    {/if}
    {#if trialAvailable}
      <Text tone="muted">Just looking? <Link href={accountsTrialUrl()}>Try it without an account</Link></Text>
    {/if}
  {/if}
</Page>

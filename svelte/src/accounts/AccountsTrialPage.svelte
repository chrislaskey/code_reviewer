<!-- "Try it now", at /accounts/trial, on the web: the app for a day with
     no sign-up, as a trial user, see
     docs/design/accounts-local-and-trial-users.md. The trial starts from
     the button, never from opening the page. -->
<script>
  import { accountsTrialCreate, accountsTrialGet } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentUserForget } from "../lib/current_user.svelte.js";
  import { navigateToUrl } from "../lib/router.js";
  import { accountsSignInUrl, accountsSignUpUrl, postsIndexUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Link, Page, Text } from "../lib/components/index.js";

  /** Whether a trial can start; null until the server answered. */
  let available = $state(/** @type {boolean | null} */ (null));
  let error = $state("");
  let busy = $state(false);

  accountsTrialGet().then(
    (answer) => (available = answer),
    (problem) => (error = errorMessageOf(problem)),
  );

  async function start() {
    busy = true;
    error = "";
    try {
      await accountsTrialCreate();
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>Try it now</Heading>
  {#if available === false}
    <Text>Trials are not available right now.</Text>
    <Text><Button variant="primary" href={accountsSignUpUrl()}>Sign up</Button></Text>
  {:else}
    <Text>
      Use the app for 24 hours with no account. What you make is deleted after that, unless you sign up, which
      keeps all of it.
    </Text>
    <Text tone="muted" size="sm">A trial stays in this browser, and has no files or picture until you sign up.</Text>
    <ErrorMessage message={error} />
    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" onclick={start} disabled={busy || available === null} data-trial>
        Start a trial
      </Button>
      <Link size="sm" href={accountsSignUpUrl()}>Sign up</Link>
      <Link size="sm" href={accountsSignInUrl()}>Sign in</Link>
    </div>
  {/if}
</Page>

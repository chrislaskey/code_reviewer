<!-- "Try it now", at /accounts/trial, on the web: the app for a day with
     no sign-up, as a trial user, see
     docs/design/accounts-local-and-trial-users.md. The trial starts from
     the button, never from opening the page. -->
<script>
  import { t } from "ttag";
  import { accountsTrialCreate, accountsTrialGet } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentUserForget } from "../lib/current_user.svelte.js";
  import { navigateToUrl } from "../lib/router.js";
  import { accountsSignInUrl } from "../lib/routes.js";
  import { pageAfterSignInUrl } from "../lib/pages.js";
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
      await navigateToUrl(pageAfterSignInUrl());
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>{t`Try it now`}</Heading>
  {#if available === false}
    <Text>{t`Guest visits are not available right now.`}</Text>
    <Text><Button variant="primary" href={accountsSignInUrl()}>{t`Log in or sign up`}</Button></Text>
  {:else}
    <Text>
      {t`Use the app for 24 hours with no account. What you make is deleted after that, unless you sign up, which keeps all of it.`}
    </Text>
    <Text tone="muted" size="sm">{t`A guest stays in this browser, and has no files or picture until they sign up.`}</Text>
    <ErrorMessage message={error} />
    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" onclick={start} disabled={busy || available === null} data-trial>
        {t`Continue as guest`}
      </Button>
      <Link size="sm" href={accountsSignInUrl()}>{t`Log in or sign up`}</Link>
    </div>
  {/if}
</Page>

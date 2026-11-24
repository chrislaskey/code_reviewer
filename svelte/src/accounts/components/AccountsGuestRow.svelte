<!-- "Continue as guest" on the Log in page: a trial started at once, the
     app for a day with no sign-up; the account menu explains it once
     they are in, see docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { t } from "ttag";
  import { accountsTrialCreate } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { currentUserForget } from "../../lib/current_user.svelte.js";
  import { navigateToUrl } from "../../lib/router.js";
  import { postsIndexUrl } from "../../lib/routes.js";
  import { ErrorMessage, Icon } from "../../lib/components/index.js";

  let error = $state("");
  let busy = $state(false);

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

<button
  type="button"
  class="rounded-box bg-primary/10 text-primary flex w-full cursor-pointer items-center justify-between gap-3 px-4 py-3 text-left disabled:opacity-60"
  onclick={start}
  disabled={busy}
  data-trial
>
  <span>
    <span class="block font-semibold">{t`Continue as guest`}</span>
    <span class="muted block text-sm">{t`No sign-up required`}</span>
  </span>
  <Icon name="hero-chevron-right" />
</button>
<ErrorMessage message={error} />

<!-- Links to the terms and the privacy policy, public pages: a new tab on
     the web, the system browser in the apps, which never show them. -->
<script>
  import { jt, t } from "ttag";
  import { publicPagesOpen } from "./core/index.js";
  import { Link, TranslatedParts } from "./components/index.js";

  /** @param {MouseEvent} event @param {"terms" | "privacy"} page */
  function open(event, page) {
    event.preventDefault();
    publicPagesOpen(page);
  }
</script>

{#snippet termsLink()}<Link href="/terms" onclick={(event) => open(event, "terms")}>{t`Terms of Service`}</Link>{/snippet}
{#snippet privacyLink()}<Link href="/privacy" onclick={(event) => open(event, "privacy")}>{t`Privacy Policy`}</Link>{/snippet}
<TranslatedParts parts={jt`${termsLink} and ${privacyLink}`} />

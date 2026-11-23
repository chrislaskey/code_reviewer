<!-- Links to the terms and the privacy policy, public pages: a new tab on
     the web, the system browser in the apps, which never show them; in
     the language the pages show. -->
<script>
  import { jt, t } from "ttag";
  import { publicPagesOpen } from "./core/index.js";
  import { translationsShown } from "./translations.js";
  import { Link, TranslatedParts } from "./components/index.js";

  const locale = translationsShown();
  const prefix = locale === "en" ? "" : `/${locale}`;

  /** @param {MouseEvent} event @param {"terms" | "privacy"} page */
  function open(event, page) {
    event.preventDefault();
    publicPagesOpen(page, locale);
  }
</script>

{#snippet termsLink()}<Link href={`${prefix}/terms`} onclick={(event) => open(event, "terms")}>{t`Terms of Service`}</Link>{/snippet}
{#snippet privacyLink()}<Link href={`${prefix}/privacy`} onclick={(event) => open(event, "privacy")}>{t`Privacy Policy`}</Link>{/snippet}
<TranslatedParts parts={jt`${termsLink} and ${privacyLink}`} />

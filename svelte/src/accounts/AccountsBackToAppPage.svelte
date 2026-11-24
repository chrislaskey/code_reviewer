<!-- Where Stripe's pages come back to when the desktop app opened them,
     and where the desktop app's sign-in listener sends the browser, at
     /accounts/back-to-app: open to anyone, as this browser may not be
     signed in. The app asks the server again when it has the focus. See
     docs/design/purchasing.md, step 7, and accounts-sign-in.md. -->
<script>
  import { t } from "ttag";
  import { Heading, Page, Text } from "../lib/components/index.js";
</script>

<Page>
  <Heading level={1}>{t`All done here`}</Heading>
  <Text>{t`Go back to the app. It shows what changed when you do.`}</Text>
  <Text tone="muted">{t`You can close this tab.`}</Text>
</Page>

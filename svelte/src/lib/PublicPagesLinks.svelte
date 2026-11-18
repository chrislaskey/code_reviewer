<!-- Links to the terms and the privacy policy, public pages: a new tab on
     the web, the system browser in the apps, which never show them. -->
<script>
  import { publicPagesOpen } from "./core/index.js";
  import { Link } from "./components/index.js";

  /** @param {MouseEvent} event @param {"terms" | "privacy"} page */
  function open(event, page) {
    event.preventDefault();
    publicPagesOpen(page);
  }
</script>

<Link href="/terms" onclick={(event) => open(event, "terms")}>Terms of Service</Link> and
<Link href="/privacy" onclick={(event) => open(event, "privacy")}>Privacy Policy</Link>

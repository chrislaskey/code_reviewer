<!-- The ops app's landing page, at /ops: a placeholder until ops is
     built. Only staff are given the page, see axum/src/ops.rs. -->
<script>
  import { homeUrl } from "../lib/routes.js";
  import { opsCurrentStaffGet } from "./api.js";
  import { ErrorMessage, Heading, Link, Text } from "../lib/components/index.js";
  import { errorMessageOf } from "../lib/error_message.js";

  /** @type {{user_id: string, inserted_at: string} | null} */
  let staff = $state(null);
  let error = $state("");

  opsCurrentStaffGet().then(
    (answer) => (staff = answer),
    (problem) => (error = errorMessageOf(problem)),
  );
</script>

<main class="page">
  <nav class="flex flex-wrap items-center gap-4">
    <span class="font-semibold">Ops</span>
    <span class="grow"></span>
    <Link href={homeUrl()}>Back to the app</Link>
  </nav>
  <Heading level={1}>Ops</Heading>
  {#if error}
    <ErrorMessage message={error} />
  {:else if staff}
    <Text tone="muted">Nothing here yet.</Text>
  {/if}
</main>

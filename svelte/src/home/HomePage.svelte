<!-- The app's home page, at /home: its features, and in a browser, which
     only signs in, that they are in the apps. -->
<script>
  import { isTauri } from "@tauri-apps/api/core";
  import { Alert, Page } from "../lib/components/index.js";
  import Header from "../lib/code_reviewer/components/Header.svelte";
  import StillWaves from "../lib/code_reviewer/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/code_reviewer/components/TopBackgroundPattern.svelte";
  import { reviewsNewUrl, sessionsIndexUrl, tracesNewUrl } from "../lib/routes.js";
</script>

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<Page wide class="with-waves">
  <Header>
    Code Reviewer
    {#snippet subtitle()}
      Run Claude Code, Cursor or Codex agent sessions in tmux, review what a change did, and trace a test through it.
    {/snippet}
  </Header>

  {#if !isTauri()}
    <Alert class="mb-6">
      Sessions, reviews and traces run in Code Reviewer's apps for the Mac and the iPhone. In a browser you can only
      manage your account.
    </Alert>
  {/if}

  <section>
    <a class="raised-panel feature" href={sessionsIndexUrl()}>
      <h2 class="section-heading">Sessions</h2>
      <p>The agent sessions started here</p>
      <p class="muted">
        See whether the agent is busy or idle in each tmux session this app started, open one in your terminal app, or
        start a new one.
      </p>
    </a>
  </section>

  <section>
    <a class="raised-panel feature" href={reviewsNewUrl()}>
      <h2 class="section-heading">Reviews</h2>
      <p>Review the changes of a repository</p>
      <p class="muted">
        Pick a git repository with Elixir or Rust code and what to compare: uncommitted changes, one commit, or one
        branch against another.
      </p>
    </a>
  </section>

  <section>
    <a class="raised-panel feature" href={tracesNewUrl()}>
      <h2 class="section-heading">Traces</h2>
      <p>Trace a test through the changes</p>
      <p class="muted">
        Pick a test and read every call below it, top to bottom like a stack trace, with each function marked against
        the base of the comparison.
      </p>
    </a>
  </section>
</Page>

<style>
  .feature {
    display: block;
    padding: 1rem 1.25rem;
    color: inherit;
    text-decoration: none;
  }

  .feature h2 {
    margin-bottom: 0.35rem;
  }
</style>

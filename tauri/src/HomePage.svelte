<!-- The first page: the app's features. -->
<script>
  import AppNav from "./lib/components/AppNav.svelte";
  import Header from "./lib/components/Header.svelte";
  import StillWaves from "./lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "./lib/components/TopBackgroundPattern.svelte";
  import { reviewNewUrl, sessionsIndexUrl, traceNewUrl } from "./lib/routes.js";
</script>

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="home" />

<main class="page with-waves">
  <Header>
    Code Reviewer
    {#snippet subtitle()}
      Run Claude Code or Cursor agent sessions in tmux, review what a change did, and trace a test through it.
    {/snippet}
  </Header>

  <section>
    <a class="panel feature" href={sessionsIndexUrl()}>
      <div class="feature-text">
        <h2>Sessions</h2>
        <p>The agent sessions started here</p>
        <p class="muted">See whether the agent is busy or idle in each tmux session this app started, open one in your terminal app, or start a new one.</p>
      </div>
    </a>
  </section>

  <section>
    <a class="panel feature" href={reviewNewUrl()}>
      <div class="feature-text">
        <h2>Reviews</h2>
        <p>Review the changes of a repository</p>
        <p class="muted">Pick a git repository with Elixir or Rust code and what to compare: uncommitted changes, one commit, or one branch against another.</p>
      </div>
    </a>
  </section>

  <section>
    <a class="panel feature" href={traceNewUrl()}>
      <div class="feature-text">
        <h2>Traces</h2>
        <p>Trace a test through the changes</p>
        <p class="muted">Pick a test and read every call below it, top to bottom like a stack trace, with each function marked against the base of the comparison.</p>
      </div>
    </a>
  </section>
</main>

<style>
  .feature {
    display: block;
    padding: 1rem 1.25rem;
    color: inherit;
    text-decoration: none;
    cursor: pointer;
  }

  .feature-text {
    min-width: 0;
  }

  .feature-text h2 {
    margin-bottom: 0.35rem;
  }
</style>

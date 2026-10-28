<!-- The first page: the app's features. -->
<script>
  import AppNav from "./lib/components/AppNav.svelte";
  import Header from "./lib/components/Header.svelte";
  import { reviewNewUrl, sessionsIndexUrl, traceNewUrl } from "./lib/routes.js";
</script>

<AppNav current="home" />

<main class="page">
  <Header>
    Code Reviewer
    {#snippet subtitle()}
      Review what a change did, trace a test through it, and run Claude Code sessions in tmux.
    {/snippet}
  </Header>

  <section>
    <h2>Reviews</h2>
    <p><a href={reviewNewUrl()}>Review the changes of a repository →</a></p>
    <p class="muted">Pick a git repository with Elixir or Rust code and what to compare: uncommitted changes, one commit, or one branch against another.</p>
  </section>

  <section>
    <h2>Traces</h2>
    <p><a href={traceNewUrl()}>Trace a test through the changes →</a></p>
    <p class="muted">Pick a test and read every call below it, top to bottom like a stack trace, with each function marked against the base of the comparison.</p>
  </section>

  <section>
    <h2>Sessions</h2>
    <p><a href={sessionsIndexUrl()}>The Claude Code sessions started here →</a></p>
    <p class="muted">See whether Claude Code is busy or idle in each tmux session this app started, open one in your terminal app, or start a new one.</p>
  </section>
</main>

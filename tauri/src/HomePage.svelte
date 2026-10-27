<!-- The first page: the app's two features. -->
<script>
  import AppNav from "./lib/components/AppNav.svelte";
  import Header from "./lib/components/Header.svelte";
</script>

<AppNav current="home" />

<main class="page">
  <Header>
    Code Reviewer
    {#snippet subtitle()}
      Review what a change did, and run Claude Code sessions in tmux.
    {/snippet}
  </Header>

  <section>
    <h2>Reviews</h2>
    <p><a href="/reviews/new.html">Review the changes of a repository →</a></p>
    <p class="muted">Pick a git repository with Elixir code and what to compare: uncommitted changes, one commit, or one branch against another.</p>
  </section>

  <section>
    <h2>Sessions</h2>
    <p><a href="/sessions/index.html">The Claude Code sessions started here →</a></p>
    <p class="muted">See whether Claude Code is busy or idle in each tmux session this app started, open one in your terminal app, or start a new one.</p>
  </section>
</main>

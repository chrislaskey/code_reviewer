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
    <p><a href="/sessions/index.html">Your tmux and Claude Code sessions →</a></p>
    <p class="muted">See what runs in each tmux pane, open a session in your terminal app, or start a new one running Claude Code.</p>
  </section>
</main>

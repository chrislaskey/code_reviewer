<!-- The strip across the top of every page: the app's two features and the
     theme toggle. `current` names the feature the page belongs to. -->
<script>
  import ThemeToggle from "./ThemeToggle.svelte";
  import { homeUrl, reviewNewUrl, sessionsIndexUrl } from "../routes.js";

  /** @type {{ current?: "home" | "reviews" | "sessions" }} */
  let { current = "home" } = $props();

  const LINKS = [
    { id: "reviews", label: "Reviews", href: reviewNewUrl() },
    { id: "sessions", label: "Sessions", href: sessionsIndexUrl() },
  ];
</script>

<nav class="app-nav">
  <a href={homeUrl()} class="app-name" class:current={current === "home"}>Code Reviewer</a>
  {#each LINKS as link (link.id)}
    <a href={link.href} class:current={current === link.id}>{link.label}</a>
  {/each}
  <div class="ml-auto">
    <ThemeToggle />
  </div>
</nav>

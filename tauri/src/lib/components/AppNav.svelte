<!-- The pill across the top of every page: the app's features and the
     theme menu. `current` names the feature the page belongs to. -->
<script>
  import ThemeMenu from "./ThemeMenu.svelte";
  import { homeUrl, reviewNewUrl, sessionsIndexUrl, traceNewUrl } from "../routes.js";

  /** @type {{ current?: "home" | "reviews" | "traces" | "sessions" }} */
  let { current = "home" } = $props();

  const LINKS = [
    { id: "sessions", label: "Sessions", href: sessionsIndexUrl() },
    { id: "reviews", label: "Review", href: reviewNewUrl() },
    { id: "traces", label: "Trace", href: traceNewUrl() },
  ];
</script>

<nav class="app-nav">
  <a href={homeUrl()} class="app-name" class:current={current === "home"}>Code Reviewer</a>
  {#each LINKS as link (link.id)}
    <a href={link.href} class:current={current === link.id}>{link.label}</a>
  {/each}
  <div class="ml-auto">
    <ThemeMenu />
  </div>
</nav>

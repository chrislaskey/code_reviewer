<!-- A short status beside something, such as "Active". -->
<script>
  /** @type {{ tone?: "default" | "primary" | "ghost" | "warning", class?: any, children: import("svelte").Snippet }} */
  let { tone = "default", class: class_, children } = $props();

  const TONE_CLASSES = { default: "", primary: "badge-primary", ghost: "badge-ghost", warning: "badge-warning" };
</script>

<span class={["badge badge-sm", TONE_CLASSES[tone], class_]}>{@render children()}</span>

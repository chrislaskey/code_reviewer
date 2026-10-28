<!-- The Starter Pack's button: daisyUI classes, a link when given href. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {any} [class]
   * @property {"primary"} [variant]
   * @property {import("svelte").Snippet<[]>} children
   */

  /** @type {Props & Record<string, any>} */
  let { class: class_, variant, children, ...rest } = $props();

  const variants = { primary: "btn-primary" };

  let classes = $derived(class_ ?? ["btn text-base", variants[variant] ?? "btn-primary btn-soft"]);
</script>

{#if rest.href}
  <a class={classes} {...rest}>
    {@render children()}
  </a>
{:else}
  <button class={classes} {...rest}>
    {@render children()}
  </button>
{/if}

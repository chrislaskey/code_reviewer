<!-- The Starter Pack's page header: a title, an optional subtitle and actions. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {import("svelte").Snippet<[]>} [actions]
   * @property {import("svelte").Snippet<[]>} children
   * @property {import("svelte").Snippet<[]>} [subtitle]
   */

  /** @type {Props} */
  let { actions, children, subtitle } = $props();
</script>

<header class={[actions != null && "flex items-center justify-between gap-6", "pb-6"]}>
  <div>
    <h1 class="text-2xl font-semibold leading-9">
      {@render children?.()}
    </h1>
    {#if subtitle != null}<p class="mt-1 text-base-content/70">
      {@render subtitle?.()}
    </p>{/if}
  </div>
  <div class="flex-none">{@render actions?.()}</div>
</header>

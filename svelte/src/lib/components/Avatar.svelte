<!-- A person's round picture: their avatar, else their initials, else a
     person icon. A picture that doesn't load shows the initials too, such
     as a dev server's localhost address seen from a phone. `size` is "sm"
     for the header, "lg" for the profile. -->
<script>
  import Icon from "./Icon.svelte";

  /** @type {{ user: { name?: string, avatar_url?: string | null }, size?: "sm" | "lg", class?: any }} */
  let { user, size = "sm", class: class_ } = $props();

  /** The address that didn't load; a new one is tried again. */
  let failedUrl = $state(/** @type {string | null} */ (null));

  let initials = $derived(
    (user.name ?? "")
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0].toUpperCase())
      .join(""),
  );
  let showsPicture = $derived(Boolean(user.avatar_url) && user.avatar_url !== failedUrl);
</script>

{#if showsPicture}
  <img
    src={user.avatar_url}
    alt=""
    class={["avatar-picture", `avatar-${size}`, class_]}
    onerror={() => (failedUrl = user.avatar_url ?? null)}
  />
{:else if initials}
  <span class={["avatar-initials", `avatar-${size}`, class_]}>{initials}</span>
{:else}
  <Icon name="hero-user-circle" class={[`avatar-${size}`, class_]} />
{/if}

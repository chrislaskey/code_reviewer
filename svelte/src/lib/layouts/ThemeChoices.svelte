<!-- A menu's choices of theme: "System", to follow the OS light or dark
     setting, then every daisyUI theme. A choice shows at once
     (themesFollow), calls `onchoose`, and is kept as the signed-in
     person's `theme` preference; a visitor's stays on this device. -->
<script>
  import { t } from "ttag";
  import Icon from "../components/Icon.svelte";
  import { accountsPreferencesUpdate } from "../core/index.js";
  import { currentUser } from "../current_user.svelte.js";
  import { errorMessageOf } from "../error_message.js";
  import { flashMessageShow } from "../flash_messages.svelte.js";
  import { THEMES, currentTheme, themesFollow } from "./themes.js";

  /**
   * @typedef {Object} Props
   * @property {string} [current]
   * @property {() => void} [onchoose]
   */

  /** @type {Props} */
  let { current = $bindable(currentTheme()), onchoose } = $props();

  /** @param {string} theme a daisyUI theme, or "system" */
  async function chooseTheme(theme) {
    const chosen = theme === "system" ? null : theme;
    themesFollow(chosen);
    current = currentTheme();
    onchoose?.();
    const user = currentUser.value;
    if (!user) return;
    try {
      user.preferences = await accountsPreferencesUpdate("theme", chosen);
    } catch (problem) {
      const reason = errorMessageOf(problem);
      flashMessageShow(t`Could not keep the theme: ${reason}`, { tone: "error" });
    }
  }
</script>

<li>
  <button type="button" class:menu-active={current === "system"} onclick={() => chooseTheme("system")}>
    <Icon name="hero-computer-desktop-micro" class="size-4" />
    {t`System`}
  </button>
</li>
{#each THEMES as theme (theme)}
  <li>
    <button type="button" class:menu-active={current === theme} onclick={() => chooseTheme(theme)}>
      <span data-theme={theme} class="grid grid-cols-2 gap-0.5 rounded-sm bg-base-100 p-0.5 shadow-sm">
        <span class="size-1.5 rounded-full bg-base-content"></span>
        <span class="size-1.5 rounded-full bg-primary"></span>
        <span class="size-1.5 rounded-full bg-secondary"></span>
        <span class="size-1.5 rounded-full bg-accent"></span>
      </span>
      <span class="capitalize">{theme}</span>
    </button>
  </li>
{/each}

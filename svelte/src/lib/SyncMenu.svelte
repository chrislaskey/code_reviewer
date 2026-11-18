<!-- Inside the app, in its header: the sync status as one button, which
     says what needs attention (working offline, changes not saved on the
     server, changes waiting) or shows a cloud when all is synced, and
     opens the whole status with the buttons to sync now and to work
     offline or go online. The web shows nothing: it is the server; and
     nothing for a local user, who has nothing to sync with. A sync that
     found the account deleted elsewhere, which removed it from the
     device, opens the device's accounts saying so. -->
<script>
  import { ngettext, msgid, t } from "ttag";
  import HeaderMenu from "./HeaderMenu.svelte";
  import { onSynced, syncRun, syncSetOffline, syncStatus } from "./core/index.js";
  import { currentUserForget } from "./current_user.js";
  import { navigateToUrl } from "./router.js";
  import { accountsIndexUrl, conflictsIndexUrl } from "./routes.js";
  import { Button, Icon, Link, Text } from "./components/index.js";

  let status = $state(null);
  let busy = $state(false);
  let error = $state("");

  async function load() {
    try {
      status = await syncStatus();
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {unknown} outcome */
  function synced(outcome) {
    const {
      report,
      error: failure,
      account_deleted: accountDeleted,
    } = /** @type {{report?: {push_error?: string}, error?: string, account_deleted?: boolean}} */ (outcome ?? {});
    if (accountDeleted) {
      currentUserForget();
      navigateToUrl(accountsIndexUrl({ accountDeleted: true }));
      return;
    }
    error = failure ?? report?.push_error ?? "";
    load();
  }

  /** @param {() => Promise<unknown>} action */
  async function doThen(action) {
    busy = true;
    error = "";
    try {
      await action();
      await load();
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  $effect(() => onSynced(synced));
  load();
</script>

{#if status && status.signed_in && !status.local}
  <HeaderMenu label={t`Sync`} buttonClass="gap-1.5">
    {#snippet button()}
      {#if status.offline}
        <Icon name="hero-signal-slash" class="size-5 text-warning" />
        <span class="max-sm:hidden">{t`Offline`}</span>
      {/if}
      {#if status.conflicts > 0}
        <Icon name="hero-exclamation-triangle" class="size-5 text-error" />
        <span class="max-sm:hidden">{t`${status.conflicts} not saved`}</span>
      {:else if status.pending > 0}
        <Icon name="hero-cloud-arrow-up" class="size-5" />
        <span class="max-sm:hidden">{t`${status.pending} waiting`}</span>
      {:else if !status.offline}
        <Icon name={error ? "hero-exclamation-circle" : "hero-cloud"} class={["size-5", error && "text-error"]} />
      {/if}
    {/snippet}
    {#snippet children(close)}
      <div class="flex flex-col gap-2 p-2 text-sm">
        {#if status.offline}
          <Text as="span" tone="warning">{t`Working offline`}</Text>
        {/if}
        {#if status.conflicts > 0}
          <Link tone="danger" href={conflictsIndexUrl()} onclick={close}>
            {ngettext(
              msgid`${status.conflicts} change was not saved on the server`,
              `${status.conflicts} changes were not saved on the server`,
              status.conflicts,
            )}
          </Link>
        {/if}
        {#if status.pending > 0}
          <Text as="span" tone="muted">{t`${status.pending} waiting to sync`}</Text>
        {:else if !status.offline && status.conflicts === 0}
          <Text as="span" tone="muted">{t`Everything is synced`}</Text>
        {/if}
        {#if error}
          <Text as="span" tone="error">{error}</Text>
        {/if}
        <div class="flex flex-wrap gap-2">
          {#if status.offline}
            <Button size="sm" variant="primary" onclick={() => doThen(() => syncSetOffline(false))} disabled={busy}>
              {t`Go online`}
            </Button>
          {:else}
            <Button size="sm" onclick={() => doThen(syncRun)} disabled={busy}>{t`Sync now`}</Button>
            <Button size="sm" onclick={() => doThen(() => syncSetOffline(true))} disabled={busy}>
              {t`Work offline`}
            </Button>
          {/if}
        </div>
      </div>
    {/snippet}
  </HeaderMenu>
{/if}

<!-- The end of a sign-in a device started in this browser: on a desktop
     or an iPhone, the code returned to the app at once, else the code
     the person pastes into the app, see docs/design/accounts-sign-in.md. -->
<script>
  import { Button, Text } from "../../lib/components/index.js";
  import { accountsAppReturnUrl } from "../../lib/routes.js";
  /** @type {{code: string, appReturn?: {appPort?: string, appScheme?: string}}} */
  let { code, appReturn = {} } = $props();
  let copied = $state(false);
  const returnUrl = $derived(accountsAppReturnUrl(appReturn, code));

  $effect(() => {
    if (returnUrl) window.location.assign(returnUrl);
  });

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<div class="flex flex-col gap-3">
  {#if returnUrl}
    <Text>You are logged in. Returning you to the app…</Text>
    <Text tone="muted" size="sm">If the app does not open, paste this code into it.</Text>
  {:else}
    <Text>You are logged in. Paste this code into the app to finish there, then you can close this tab.</Text>
  {/if}
  <div class="flex flex-wrap items-center gap-3">
    <code class="rounded bg-base-200 px-3 py-2 text-lg break-all select-all" data-device-code>{code}</code>
    <Button size="sm" onclick={copy}>{copied ? "Copied" : "Copy"}</Button>
  </div>
  <Text tone="muted" size="sm">The code works once and for five minutes.</Text>
</div>

<!-- "Custom server" on the accounts page, when the build's multiServer
     setting allows a server other than the app's own, see
     docs/design/multi-server.md: the server's address, checked, then
     shown with its host, its name and whether the app's own server lists
     it, and signed in to once the person confirms. What is allowed is the
     Rust side's to say; this only shows it. -->
<script>
  import { deviceServersCreate, serversGet } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { Alert, Badge, Button, ErrorMessage, Input, Label, Panel, Text } from "../../lib/components/index.js";

  /** @typedef {{url: string, host: string, name: string, display_name: string, listing: "default" | "listed" | "unlisted" | "unchecked"}} ServerChecked */

  /** @type {{ busy: boolean, onsignin: (serverId: string) => unknown }} */
  let { busy, onsignin } = $props();

  let open = $state(false);
  let url = $state("");
  /** @type {ServerChecked | null} */
  let checked = $state(null);
  let error = $state("");
  let checking = $state(false);

  /** @param {SubmitEvent} event */
  async function check(event) {
    event.preventDefault();
    checking = true;
    error = "";
    checked = null;
    try {
      checked = await serversGet(url);
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      checking = false;
    }
  }

  async function signIn() {
    if (!checked) return;
    checking = true;
    error = "";
    try {
      const server = await deviceServersCreate(checked.url);
      close();
      await onsignin(server.id);
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      checking = false;
    }
  }

  function close() {
    open = false;
    url = "";
    checked = null;
    error = "";
  }
</script>

{#if !open}
  <Button size="sm" variant="ghost" onclick={() => (open = true)} disabled={busy}>Custom server</Button>
{:else}
  <Panel>
    <form class="flex flex-col gap-3" onsubmit={check}>
      <Label text="Server address">
        <Input type="url" bind:value={url} placeholder="https://example.com" autocapitalize="off" spellcheck="false" />
      </Label>
      <div class="flex flex-wrap gap-3">
        <Button size="sm" type="submit" disabled={busy || checking || !url.trim()}>Check</Button>
        <Button size="sm" variant="ghost" onclick={close} disabled={checking}>Cancel</Button>
      </div>
    </form>
    <ErrorMessage message={error} />
    {#if checked}
      <div class="flex flex-wrap items-center gap-3">
        <span class="grow">{checked.display_name} · {checked.host}</span>
        {#if checked.listing === "listed"}
          <Badge tone="primary">Listed</Badge>
        {:else if checked.listing === "default"}
          <Badge tone="primary">The app's own server</Badge>
        {/if}
      </div>
      {#if checked.listing === "unlisted"}
        <Alert tone="warning">
          This server is not on the app's list. Go on only if you trust whoever runs it: everything you make there is
          theirs to keep.
        </Alert>
      {:else if checked.listing === "unchecked"}
        <Alert tone="warning">
          The app's own server could not be reached to check this one. Go on only if you trust whoever runs it.
        </Alert>
      {/if}
      <Text tone="muted" size="sm">You log in on that server's own page, in the browser.</Text>
      <div>
        <Button size="sm" variant="primary" onclick={signIn} disabled={busy || checking}>Sign up or log in there</Button>
      </div>
    {/if}
  </Panel>
{/if}

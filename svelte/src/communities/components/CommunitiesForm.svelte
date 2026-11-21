<!-- The fields of a community, for the New and Edit pages: its name,
     bound to the page's `name`, with the messages a write came back with. -->
<script>
  import { Button, ErrorMessage, FieldErrors, Input, Label } from "../../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} name
   * @property {string} submitText
   * @property {Record<string, string[]>} [errors] a FormError's, by field
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   */

  /** @type {Props} */
  let { name = $bindable(""), submitText, errors = {}, error = "", busy = false, onsubmit } = $props();

  /** @param {SubmitEvent} event */
  function submit(event) {
    event.preventDefault();
    onsubmit();
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <Label text="Name">
    <Input bind:value={name} maxlength="80" disabled={busy} />
    <FieldErrors {errors} fields={["name"]} />
  </Label>
  {#if error && Object.keys(errors).length === 0}
    <ErrorMessage message={error} />
  {/if}
  <div>
    <Button variant="primary" type="submit" disabled={busy}>{submitText}</Button>
  </div>
</form>

<!--
  Written by hand: the Svelte counterpart of the dynamic_form library's
  renderer, for apps without LiveView. It renders a form the `dynamic_form`
  crate (native/dynamic_form) shows: the elements, with
  DynamicFormElements.svelte, then the submit button. The page decides
  what a change, a submit and an entry added or removed do, and sends the
  form's `state` back with each, as the library's LiveComponent keeps its
  changeset between events. See docs/design/dynamic-forms.md.
-->
<script>
  import Button from "./Button.svelte"
  import DynamicFormElements from "./DynamicFormElements.svelte"

  /**
   * @typedef {{ params: Record<string, any>, submitted: boolean }} FormState
   * @typedef {[string, string][]} Entries
   */

  /**
   * @typedef {Object} Props
   * @property {string} id the `<form>` element's id, which the field ids start with
   * @property {{ elements: any[], state: FormState }} form what the crate's `DynamicForm` shows
   * @property {string} [submit_text]
   * @property {boolean} [disabled] while a submit is in flight
   * @property {(entries: Entries, state: FormState) => void} [onchange]
   * @property {(entries: Entries, state: FormState) => void} [onsubmit]
   * @property {(path: string, state: FormState) => void} [onadd] an entry added to the nested form at `path`
   * @property {(path: string, index: number, state: FormState) => void} [onremove]
   */

  /** @type {Props} */
  let { id, form, submit_text = "Submit", disabled = false, onchange, onsubmit, onadd, onremove } = $props()

  const actions = {
    /** @param {string} path */
    add: (path) => onadd?.(path, form.state),
    /** @param {string} path @param {number} index */
    remove: (path, index) => onremove?.(path, index, form.state),
  }

  /** @param {HTMLFormElement} element */
  function entriesOf(element) {
    return [...new FormData(element)].map(([name, value]) => /** @type {[string, string]} */ ([name, String(value)]))
  }

  /** @param {Event & { currentTarget: HTMLFormElement }} event */
  function change(event) {
    onchange?.(entriesOf(event.currentTarget), form.state)
  }

  /** @param {SubmitEvent & { currentTarget: HTMLFormElement }} event */
  function submit(event) {
    event.preventDefault()
    onsubmit?.(entriesOf(event.currentTarget), form.state)
  }
</script>

<form {id} oninput={change} onsubmit={submit}>
  <DynamicFormElements elements={form.elements} {actions} />
  <div class="mt-6 flex items-center justify-end gap-x-6">
    <Button type="submit" {disabled}>{disabled ? "Saving..." : submit_text}</Button>
  </div>
</form>

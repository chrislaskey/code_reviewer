<!--
  Written by hand: the elements of a dynamic form, as the dynamic_form
  library's renderer writes them (Renderer.Component, ContentElements and
  NestedEntries, and its own dynamic_form_group, nested_entry and label
  components, which App.Web.CoreComponents doesn't define). Each question
  renders with the field component `mix app.svelte` mirrors from
  App.Web.CoreComponents, from the props the Rust's `view` gives it.
  Renders itself for a panel's members and a nested form's entries.
-->
<script>
  import Button from "./Button.svelte"
  import CheckboxGroupInput from "./CheckboxGroupInput.svelte"
  import CheckboxInput from "./CheckboxInput.svelte"
  import DynamicFormElements from "./DynamicFormElements.svelte"
  import Error from "./Error.svelte"
  import RadioGroupInput from "./RadioGroupInput.svelte"
  import SelectInput from "./SelectInput.svelte"
  import TextareaInput from "./TextareaInput.svelte"
  import TextInput from "./TextInput.svelte"

  /**
   * @typedef {Object} Actions
   * @property {(path: string) => void} add
   * @property {(path: string, index: number) => void} remove
   */

  /**
   * @typedef {Object} Props
   * @property {any[]} elements the `view`'s elements
   * @property {Actions} actions what the add and remove buttons do
   */

  /** @type {Props} */
  let { elements, actions } = $props()

  /** @type {Record<string, import("svelte").Component<any>>} */
  const components = { CheckboxGroupInput, CheckboxInput, RadioGroupInput, SelectInput, TextareaInput, TextInput }

  /** @param {string} type */
  function membersClass(type) {
    return type === "vertical" ? "flex flex-col gap-4 [&>*]:mb-0" : "flex flex-wrap items-center gap-4 [&>*]:mb-0"
  }

  /**
   * As LiveView's data-confirm: ask first when the entry says so.
   * @param {string} path
   * @param {{ index: number, remove: { confirm: string | null } }} entry
   */
  function remove(path, entry) {
    if (entry.remove.confirm && !window.confirm(entry.remove.confirm)) return
    actions.remove(path, entry.index)
  }
</script>

{#each elements as element}
  {#if element.kind === "field"}
    {@const Field = components[element.component]}
    <div class="mb-4">
      <Field {...element.props} />
      {#if element.description}<p class="mt-2 text-sm text-gray-500">{element.description}</p>{/if}
    </div>
  {:else if element.kind === "hidden"}
    {#each element.inputs as input}<input type="hidden" name={input.name} value={input.value} />{/each}
  {:else if element.kind === "no_choices"}
    <div class="mb-4">
      {#if element.label}
        <!-- svelte-ignore a11y_label_has_associated_control: the library's label has no control here either -->
        <label class="label mb-1">{element.label}{#if element.required && element.required_label}<span class="ml-0.5 text-red-500">{element.required_label}</span>{/if}</label>
      {/if}
      <p class="mt-2 text-sm text-gray-500">{element.text}</p>
    </div>
  {:else if element.kind === "html"}
    <div class="mb-4">{@html element.html}</div>
  {:else if element.kind === "image"}
    <div class="mb-4">
      <img src={element.src} alt={element.alt} style={element.style} class="rounded-md max-w-full" />
    </div>
  {:else if element.kind === "group"}
    <div data-dynamic-form-group={element.name}>
      {#if element.title}<h3 class="text-lg font-semibold text-gray-900 mb-4">{element.title}</h3>{/if}
      <div class={membersClass(element.group_type)}>
        <DynamicFormElements elements={element.elements} {actions} />
      </div>
    </div>
  {:else if element.kind === "nested"}
    <div class="mb-4">
      <div class="mt-6 flex items-start justify-between gap-3">
        <div class="min-w-0">
          {#if element.label}
            <h3 class="text-xl font-bold">{element.label}{#if element.required && element.required_label}<span class="ml-0.5 text-red-500">{element.required_label}</span>{/if}</h3>
          {/if}
          {#if element.description}<div class="text-gray-500">{element.description}</div>{/if}
        </div>
        {#if element.add_text}
          <div class="shrink-0">
            <Button type="button" onclick={() => actions.add(element.path)}>{element.add_text}</Button>
          </div>
        {/if}
      </div>
      <input type="hidden" name={element.empty_input} value="" />
      {#if element.no_entries_text}<p class="mt-2 text-sm italic text-gray-500">{element.no_entries_text}</p>{/if}
      {#each element.entries as entry (entry.index)}
        <div class="mt-3 rounded-lg border border-gray-200 p-4">
          <div class="flex items-start gap-3">
            <div class="min-w-0 flex-1">
              {#if entry.title}<h4 class="mb-2 text-sm font-semibold text-gray-900">{entry.title}</h4>{/if}
              {#if entry.id_input}<input type="hidden" name={entry.id_input.name} value={entry.id_input.value} />{/if}
              <DynamicFormElements elements={entry.elements} {actions} />
            </div>
            {#if entry.remove}
              <button
                type="button"
                onclick={() => remove(element.path, entry)}
                aria-label={entry.remove.label}
                title={entry.remove.label}
                class="btn btn-sm btn-square btn-ghost text-red-600"
              >
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" aria-hidden="true" class="size-4">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"
                  />
                </svg>
              </button>
            {/if}
          </div>
        </div>
      {/each}
      {#each element.errors as message}<Error>{message}</Error>{/each}
    </div>
  {/if}
{/each}

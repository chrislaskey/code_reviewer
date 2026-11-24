# Dynamic form definitions as JSON

A LiveView defines a `dynamic_form` form as a function next to it that
returns the definition, and the web app calls it. For the desktop and
mobile apps, `mix app.dynamic_forms` writes the definition of each
function marked `@universal_dynamic_form true` as a `.json` file in the
same folder, which they compile in by path and validate with
`native/dynamic_form`, by the library's rules.

```elixir
@universal_dynamic_form true
def post_form do
  %{
    "id" => "post-form",
    "elements" => [
      %{"type" => "comment", "name" => "body", "title" => "Write a post", "isRequired" => true}
    ]
  }
end
```

writes `post_form.dynamic_form.json` next to the module. Commit the
`.json` files: the other apps include them and build without Elixir.
The attribute is registered for every LiveView, live component and
`use App.Web, :html` module.

## Run

```sh
mix app.dynamic_forms          # write every file that is missing or out of date
mix app.dynamic_forms --check  # report files out of date with their form
```

It is a manual step, not part of `mix compile`.
`test/dynamic_form_json/sync_test.exs` fails `mix test`, and so
`mix check`, when a file is out of date with its function, missing, or
left over from a function no longer marked. The files are never edited
by hand.

## What a marked function may be

- One public `def` clause with no arguments. The generator calls it once,
  so it returns the same definition every time. What only the server
  knows at render time, like the user's communities as a question's
  choices, is `[]` in the definition and filled in after calling it, by
  each app: `App.Web.DynamicForms.put_choices/3` here, the crate's
  `Definition::put_choices` in Rust
- JSON data only: a map with string keys, of strings, numbers, booleans,
  `nil`, lists and maps, with the definition's keys as the library spells
  them, like `"isRequired"`. An atom or a tuple would be different data
  in the file than on the web, so it's refused
- A definition `DynamicForm.Parser.FromData` reads. The desktop app's
  Rust reads less than the library does and refuses the rest, see
  "What the Rust supports" in `docs/design/dynamic-forms.md`

Each file has the keys of every object sorted, so it only changes when
the form does.

## How it works

- `DynamicFormJson.Source` finds the marked defs in a module's source
  file, as `HeexSvelte.Source` does for components
- `DynamicFormJson` calls each one, checks it, writes the file and
  compares it for `--check`

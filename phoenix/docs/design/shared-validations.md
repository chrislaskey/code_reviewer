# Sharing data validations between Elixir and Rust

A spike, October 2026, and what it settled. The question: the rules of a
CRUD app live in Ecto changesets, and the desktop and mobile apps save to
a local SQLite database in Rust first and sync later. Can the Ecto
validations run in Rust, the way `mix app.svelte` lets HEEx components
run in Svelte, and should the Elixir side then run the Rust too?

What shipped is `priv/rust`: a changeset function marked
`@universal_changeset true` is written as Rust next to its schema, the
other apps compile that Rust in, the Phoenix app runs it too, and a
repository with no mark needs no Rust toolchain. How to use it is in
`priv/rust/README.md`. This document is the record of how it got there:
the ideas weighed, the four approaches built and then removed, the
blockers each met, and what each would take to build again.

## Ideas considered

Breadth first. The verdict column says what the spikes settled.

| # | Idea | How it would work | For | Against | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | **Ecto source to Rust** | Parse the marked function's AST, emit the same pipeline on a Rust `Changeset`, as `mix app.svelte` does for HEEx | Sees messages, order and every call exactly. Output is plain Rust a person can read and step through | Only follows what it can parse: literals, pipes, helpers that are pipes. An `if`, a `case` or an option decided at runtime stops it. Has to know Elixir | Built. This is the transpiler that shipped |
| 2 | **Running changeset to JSON rules** | Call the function with empty params, read `required`, `validations`, `constraints`, `types` off the changeset. Rust interprets the JSON | No parsing. Helpers, options and branches are followed because the code runs. Same JSON can feed any other language | Sees only the path the probe takes. Ecto does not record the cast list or some messages. Custom `add_error` logic is invisible | Built, removed. Below |
| 3 | **JSON Schema** | Emit a JSON Schema from the changeset, validate with the `jsonschema` crate and a JSON Schema library in Elixir | Standard, tooling, editors | Messages and semantics differ from Ecto's: graphemes, empty values, confirmation, acceptance have no JSON Schema form. Two engines to keep in line with Ecto, not one | Not built. Idea 2 is the same shape with Ecto's own vocabulary |
| 4 | **SurveyJS JSON, the `dynamic_form` format** | Forms are already data; port `DynamicForm.Changeset` to Rust and validate the form definition | The definition travels with the form, and the Elixir side already exists | Form-centric, not schema-centric. Needs the expression engine for `visibleIf`, and nested forms | Built as a mapping onto idea 2, removed. Below |
| 5 | **Rust is the only implementation, run in Elixir by NIF** | Rustler NIF around the crate; the changeset function becomes `cast/3` plus one NIF call | One implementation, one set of tests, no drift | `mix compile` and every release build need the Rust toolchain. Elixir still casts for the repo. Loses Ecto's `validate_*` ergonomics and the LiveView form conventions built on them | Built, then reshaped into what shipped. Below |
| 6 | **Rust compiled to WASM** | The same crate as WASM, run by `wasmex` in Elixir, natively in Tauri, and in the browser for live form validation | One artifact in three places, including the webview | Same toolchain cost as 5 plus a WASM runtime in the BEAM. Browser use duplicates what LiveView already does over the socket | Not built. Worth a look only if client-side validation without a round trip becomes a goal |
| 7 | **An Elixir validation DSL** | A macro such as `validations do required [:email] ... end` that expands to Ecto calls and writes the JSON at compile time | No guessing, no probing, the rules are data from the start | A new dialect to learn and maintain; custom logic still escapes it; every existing changeset is rewritten | Not built. Idea 2 gets the same data without a dialect |
| 8 | **Contract fixtures only** | Export nothing; generate (params, errors) cases from Ecto and hold hand-written Rust to them | Smallest possible machinery; works with any approach | Rust is written twice by hand, and the fixtures only catch what someone thought to test | Built as the test harness of every spike; survives as the parity test |
| 9 | **An expression language** | JSONLogic or CEL for cross-field rules the vocabulary cannot say | Portable custom logic | Another language, immature Elixir implementations, and the rules in this codebase did not need it | Not built. Revisit when a rule needs it |

## What shipped, and why that shape

**The `.rs` next to the schema is the artifact.** Idea 1, the
transpiler. It reads the marked function's clause, follows the pipe into
helpers of the module, resolves module attributes, and writes the same
chain on the crate's `Changeset`. It sees messages and order exactly and
the output is Rust a person reads. It stops, with the file and line, on
anything it cannot follow: an `if`, an option read at runtime, a helper
with logic of its own. That is the right outcome: no transpiler should
guess at `maybe_hash_password/2`, and the function stays unmarked.

**The Phoenix app runs the Rust too, opt in.** Idea 5 reshaped. The
objection to the NIF as first built was threefold, and each part has an
answer in what shipped:

| Spike 3, the NIF as first built | What shipped |
| --- | --- |
| `mix compile` and every release need cargo | only after the first `@universal_changeset true`. The module that says `use Rustler` is generated while a function is marked and removed when none is; the Dockerfile installs the toolchain only when that module exists |
| Elixir casts for the repo, the crate casts again | still both, now on purpose: the template runs for the typed changes and the metadata the repo and `phoenix_ecto` read, the Rust for the errors. The errors come back as Ecto keeps them, message template and options, so forms and `errors_on/1` see no difference |
| the rules leave the pipeline Phoenix conventions look for | the pipeline stays in the source as the template the Rust is written from, and is what a reviewer reads |

**The parity test is the guarantee.** Idea 8. Every marked function has
cases, each run through the template and the Rust, and any difference
fails `mix test`. This is what turned "the Rust matches" into a list of
exactly which cases do not, and it found every semantic mistake made
along the way: the raw params in `validate_confirmation`, graphemes
versus bytes, the first failing `validate_number` check, blank strings
as missing, a cast error reported twice, the options Ecto keeps with a
message, and a changeset passed as data by the sync server.

**The form decoder is shared too.** Phoenix turns `tags[]=vip` into a
list through `Plug.Conn.Query` before any changeset runs, and the desktop
app has to produce the same map from its `FormData` or the rules see
different params. The crate's `form` module ports Plug's decoder, and its
tests assert what Plug returned for the same entries. See the section on
the query decoder below.

### Casting: why the Rust casts from raw params

Handing the Rust the typed changes Ecto already made would be cheaper,
and was the first idea. It would mean the web app casts with Ecto and the
desktop app casts with the crate, and any difference between the two
casts makes the same form fail in one app and pass in the other. So the
crate's cast runs in both apps on the same raw input, and Ecto's cast is a
conversion for its own database. The parity test holds the two casts
together: an error Ecto's cast reports that the Rust's does not shows up
as a difference.

### What the app sets, and what the form sends

`App.Data.Post.changeset/3` puts the owner from the scope and the sync
time, and requires the owner. The Rust writes nothing for
`put_change/3`: a put field is data to the Rust, read from `data` as the
row's other columns are. The Phoenix runtime passes the template's
changes that the params did not supply as data, and the desktop app puts
the owner in `data` itself. So a put field may be required but not
otherwise validated, since Ecto would validate the put value and the
Rust has nothing to validate; the transpiler refuses that, and a put
field that is also cast.

A helper with two clauses, `put_owner(changeset, nil)` and
`put_owner(changeset, owner_id)`, is followed because both clauses
give the same Rust: nothing. A helper whose clauses give different rules
is refused with its line.

### The sync server passes a changeset as data

`App.Data.Sync` decodes a pushed row by casting every column, then calls
the schema's changeset on that changeset. `Ecto.Changeset.cast/4` with a
changeset as data merges the earlier changes and validates a cast field's
earlier change again when the params leave it out. The runtime does the
same: an earlier change to a cast field is a param to the Rust, and the
rest of the earlier changes are data. The loader carries each function's
cast list for this, read off the transpiled steps.

## Spike 2: running changeset to JSON, removed

`EctoRules.Export` called the function with an empty struct and empty
params and read the changeset. `Ecto.Changeset` keeps `required`,
`validations` as `{field, {:length, opts}}` and the like, `constraints`
and `types`, which is most of what is needed, and `phoenix_ecto` relies
on the same data for HTML5 attributes. The result was a `.rules.json`
per schema in a vocabulary the crate interpreted, so a consumer without
Rust could read the rules too.

Two things Ecto does not keep, and the workaround for each:

* **The cast list.** `cast/3` leaves no trace. The function was run again
  with a typed probe value in every field; the fields that came back as
  changes equal to their probe were the cast ones. A field whose default
  equals the probe would be missed.
* **Messages of `validate_format`, `validate_inclusion`,
  `validate_exclusion` and `validate_subset`**, which Ecto captures in a
  closure. The function was run again with a value that fails the rule
  and the error's message read back. Two rules of the same kind on one
  field would both fire, so the message was only taken when exactly one
  did.

What stayed invisible: logic behind a branch, such as
`validate_length(:password, max: 72, count: :bytes)` only on a valid
changeset, and `add_error(:email, "did not change")`. The export could
not see them, and the fixtures carried them as cases marked
`parity: false`, reported on every run. Options were another edge: the
function was called with its defaults, so `registration_changeset` was
exported with `validate_unique: true`, which queries the repo during
probing, so the export ran with the app started and the sync test under
the SQL sandbox.

Side findings that still apply: a regex compiled twice is two terms, so
deduplicating validations compares source and flags; map key order in
Elixir is atom creation order, so generated files need a fixed key order
to be stable in git; `~r/.../i` becomes `(?i)...` for the `regex` crate.

Why it was removed: the transpiler sees everything the export saw on the
functions worth marking, and sees it exactly, while the export's two
probe runs are a trick a reader has to learn and its blind spots are
silent. The JSON would earn its place again for a consumer that cannot
compile Rust, a form builder or a sync service in another language. To
rebuild it: one module reading `required`, `validations`, `constraints`
and `types` off the running template, the two probe runs above, an
ordered JSON encoder, and an interpreter in the crate that maps each
rule onto the same `Changeset` methods the generated Rust calls. About
500 lines of Elixir and 250 of Rust.

## Spike 3: Rust in Elixir by NIF, reshaped

A Rustler crate wrapped the crate's interpreter: rules, data and params
in as JSON, errors out. `App.Data.Rules.validate_changeset/2` added them
to a changeset with the `validation:` tags Ecto uses, so `phoenix_ecto`
forms showed them unchanged. On every case it gave the errors the Ecto
functions gave. What it cost is the left column of the table above, and
what shipped is the right one. Two mechanics from it survive: the NIF
takes JSON in and returns JSON out, which keeps the Rust side free of
Rustler types, and the loader module is the only thing that says
`use Rustler`, so generating it is what makes the toolchain opt in.

Blockers met on the way to the opt-in form, and what was done:

* **Cargo runs during `mix compile` only because a module says
  `use Rustler`.** The hex package itself is Elixir. So the loader is
  generated while a function is marked and removed when none is.
  Verified by unmarking the function, running `mix app.rust`, and
  compiling and testing from clean with a `cargo` on the path that fails
  when invoked.
* **A loader without its crate cannot compile**, and the generator
  compiles the app before it writes anything. So the crate is written
  before the loader, and only once every `.rs` is.
* **Mix recreates a removed source as an empty file.** Its compiler
  touches a source whose external resources changed or whose module asks
  to be recompiled, which the loader's does through Rustler, and
  `File.touch` creates the file when it is gone. The generator compiles
  once after the removal, so the manifest forgets the module, then removes
  the empty file. The sync test reports any loader left without a mark.
* **The compiler warns on a call to a module that does not exist yet.**
  The runtime reaches the generated module through `apply`, so a
  repository between marking a function and running `mix app.rust`
  compiles without a warning.
* **rustfmt resolves a `#[path]` module relative to the file it formats**,
  so the NIF crate's `lib.rs` is formatted in its own directory, not a
  temp file.
* **Error field atoms.** `body_confirmation` is not an atom until a
  validation makes it one, so errors use `String.to_atom`.
* **`defoverridable` and a function with a default argument.** The
  override is defined for the full arity only; the clause Elixir
  generates for the default calls it.

## Spike 4: SurveyJS forms, removed

`dynamic_form` turns SurveyJS JSON into an Ecto changeset. The crate did
the same into the rule set of spike 2: question types to Ecto types as
`DynamicForm.Changeset` maps them, `isRequired`, and the `text`,
`numeric`, `email` and `regex` validators. On twelve cases generated by
running `dynamic_form` itself it matched on eleven. The twelfth was
`visibleIf`: a required question that only shows when another has a
value. The crate listed it as unsupported and did not require it;
`dynamic_form` requires it once visible.

So extending `dynamic_form` to Rust is a port of its expression engine,
`DynamicForm.Visibility`, and its nested forms, not of its validators,
which were a morning. Worth doing if forms become the main way data is
defined; the validators then come for free through the crate's
`Changeset`. One thing noticed on the way: `dynamic_form` does not
validate a dropdown's value against its `choices`, so neither did the
mapping. Removed with spike 2, whose rule set it produced; to rebuild it,
map the form definition straight onto `Changeset` calls instead.

## The Phoenix query decoder, and the shape of params

A browser posts a form as `application/x-www-form-urlencoded`, a flat
list of `name=value` pairs. Phoenix never hands that list to a changeset:
`Plug.Parsers` runs `Plug.Conn.Query.decode/1` on it, and the changeset
gets the nested map it produces. The rules of that decoder, with what the
crate's `form` module reproduces:

| Entries | Params |
| --- | --- |
| `name=Ada` | `%{"name" => "Ada"}` |
| `tags[]=vip&tags[]=partner` | `%{"tags" => ["vip", "partner"]}`, in order |
| `post[body]=Hi&post[tags][]=vip` | `%{"post" => %{"body" => "Hi", "tags" => ["vip"]}}` |
| `name=first&name=last` | `%{"name" => "last"}`, the last wins |
| `people[][name]=Ada&people[][age]=36` | `%{"people" => [%{"name" => "Ada", "age" => "36"}]}`, one map |
| `tags[]=vip&tags[x]=1` | `%{"tags" => %{"x" => "1"}}`, the last entry decides list or map |
| `odd[=1` or `[name]=2` | plain keys, the brackets are not parsed |
| more than 32 nested brackets | an error |

Every value is a string, and the input names are what the HEEx `<.input>`
components emit: `post[body]` for a form named `post`. The changeset's
`cast/3` is what turns the strings into typed values, which is why the
cast has to be shared too.

The decoder is the one piece of Phoenix between the form and the
changeset, and there are two ways for a desktop app to match it:

* **Share it.** The desktop app takes the form's entries, `FormData` in
  the webview, and passes them to the crate's decoder before the rules.
  The entries are the same list the browser would have posted, and the
  crate produces the same map Plug would. This is what the crate offers.
* **Bypass it.** Post JSON instead of a form body. A `fetch` with
  `Content-Type: application/json` reaches the changeset with the JSON
  object as is, and the desktop app can serialise the same object.
  Nothing to share, but the shape then depends on the JavaScript that
  builds the object, which is the problem moved rather than solved: a
  `FormData` to JSON helper has to invent the bracket rules again, and
  LiveView forms post through the socket as the same flat pairs Plug
  decodes. Sharing the decoder is the smaller surface.

Either way, the invariant the tests hold is from entries to errors: the
same entries give the same errors in both apps. The decoder tests in the
crate hold the first half, Plug's answers for a set of entries; the
parity test holds the second, Ecto's answers for a set of params.

## Blockers and workarounds, in one place

| Blocker | Where | Workaround |
| --- | --- | --- |
| PCRE versus the `regex` crate: no lookaround, no backreferences | crate | Pattern fails to compile and reports as an error on the field, never skipped silently. `fancy-regex` would close the gap at some cost |
| `validate_length` counts graphemes | crate | `unicode-segmentation`; `count: :bytes` and `:codepoints` handled |
| `Decimal` | crate | compared as `f64`; fine for bounds, wrong for money equality |
| Dates and times | crate | checked for shape only; a `chrono` parse would check the calendar |
| Errors of one `validate_required` come in field order from Ecto, reversed from the Rust | tests | compared sorted |
| Validations run only on non-null changes | crate | `Changeset::cast` takes `data` and keeps only changes, as Ecto |
| `validate_required` skips a field with an error already | crate | copied; otherwise a bad cast reports twice |
| Ecto's empty values: blank once trimmed, dropped from lists | crate | copied, `:binary` fields excepted as in Ecto |
| Ecto keeps options with a message, `count`, `kind`, `type`, `number`, `enum` | crate | errors carry them in Ecto's order; the runtime puts the schema's type on a cast error |
| `Ecto.Enum` and `Ecto.UUID` fields | transpiler, crate | cast by value list and by shape; any other custom type refuses the mark |
| A range in `validate_inclusion` | transpiler | refused: Ecto keeps the range in the error, the Rust a list |
| `rustfmt` versus generated text | transpiler | run `rustfmt` in the generator, in the destination directory |
| Regex structs not equal across runs | transpiler | compared by source and flags |
| A function with a default argument | runtime | the override takes the full arity; Elixir's default clause calls it |
| A changeset as data, from the sync server | runtime | its earlier cast changes are params, the rest data; the loader carries the cast list |
| NIF needs cargo at compile and release time | loader | generated only while a function is marked; the Dockerfile follows it |
| Mix recreates a removed loader as an empty file | generator | compile once after removing, then remove again |
| A loader whose crate names a renamed function cannot compile | generator | delete the loader and run `mix app.rust` again |
| `@universal_changeset` warns as unused | schema | `use App.Data.UniversalChangeset` registers it, as `@universal_component` is registered |
| A template pipes into another marked function | runtime | the captured body is rewritten to call the other template |

## Pros and cons, by approach

**Source to Rust.** Exact and readable; the generated file is ordinary
Rust. Falls over on any logic, and the answer is to not mark that
function. Needs the Rust toolchain in the Phoenix repository for
formatting. Best when the marked changesets are clean pipelines, which
`phx.gen` output mostly is and `phx.gen.auth` output is not.

**Running changeset to JSON.** Follows real code paths without parsing,
and one JSON serves Rust today and anything else later. Blind to branches
and to option-dependent behaviour, and two probe runs are a trick a
reader has to learn. Worth rebuilding for a consumer that cannot compile
Rust.

**Rust in Elixir by NIF, always on.** One implementation, no drift by
construction. A heavy toolchain and release cost, double casting, and it
moves the rules out of the pipeline where every Phoenix convention looks
for them.

**SurveyJS as the shared layer.** Right when the form is the source of
truth. For schema-defined data it is a detour, and the expression engine
is the real port.

**Fixtures.** Cheap, catches real mistakes, and the only approach that
also covers Rust written by hand. Not sufficient alone: it is only as good
as the cases.

**Rust as the implementation, opt in.** What shipped. One cast and one
set of rules in every app, from a template that still reads as an Ecto
pipeline, and no toolchain for a repository that has not opted in. The
costs are the double cast, which is the price of typed changes for the
repo, and the toolchain in every environment once anyone opts in. Gettext
works because the errors come back as Ecto keeps them. The open edges are
custom `Ecto.Type`s beyond `Ecto.Enum` and `Ecto.UUID`, and the standalone
Burrito binary, which cannot carry the NIF.

## Next

1. Mark a changeset when the desktop or mobile app writes that data, and
   add its cases to the parity test. `App.Data.Post.changeset/3` is marked;
   `App.Data.Community.changeset/3` is next when a client creates one.
2. A function with logic of its own, like `App.Data.User.email_changeset/3`,
   stays unmarked. If the other apps need its rules, split the plain
   pipeline into a marked function and keep the logic around it.
3. Pass form entries through the crate's decoder in the desktop app, not a
   JavaScript helper, so the rules see what Plug would give. The desktop
   app's post form passes a body string today, which is the same thing
   for one field.
4. If a consumer without Rust appears, rebuild the JSON export of spike 2
   on top of the transpiled steps, which already know the cast list and
   every rule, rather than on probing.
5. If forms become the way data is defined, map SurveyJS definitions
   straight onto `Changeset` calls; the validators are already there.

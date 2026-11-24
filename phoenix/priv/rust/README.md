# Universal changesets

The desktop and mobile apps save to a local SQLite database in Rust before
syncing, and have to refuse what the server would refuse. The rules live
in Ecto changeset functions. A changeset function marked
`@universal_changeset true` is written as Rust next to its schema by
`mix app.rust`, the other apps compile that Rust in, and this app runs it
too, so a form reaches the same rules with the same result in every app.

```elixir
use App.Data.UniversalChangeset

@universal_changeset true
def changeset(post, params, options \\ []) do
  post
  |> cast(params, [:body])
  |> put_owner(Keyword.get(options, :owner_id))
  |> validate_required([:owner_id, :body])
  |> validate_length(:body, max: 10_000)
  |> put_change(:last_synced_at, DateTime.utc_now())
end
```

writes `post.rs` in the schema's folder: the function as a chain of calls
on the `native/ecto_changeset` crate's `Changeset`, which has Ecto's cast
and validations with Ecto's semantics, messages and error options. Commit
the `.rs` file: the other apps include it by path and build without
Elixir.

## Run

```sh
mix app.rust          # write every file that is missing or out of date
mix app.rust --check  # report files out of date with their functions
```

It is a manual step, not part of `mix compile`. `test/ecto_rust/sync_test.exs`
fails `mix test`, and so `mix check`, when a file is out of date with its
function, missing, or left over from a function no longer marked.

## What runs in this app

The Ecto pipeline stays in the source as the template the Rust is written
from, and `App.Data.UniversalChangeset` replaces the function at compile
time with one that:

1. runs the template, for the typed changes `Repo.insert` needs, the
   constraints that turn a duplicate into an error, and the `required`
   and `validations` metadata `phoenix_ecto` reads for HTML5 attributes
2. sends the raw params to the Rust function, which casts and validates
   them as it does in the other apps, and replaces the template's errors
   with the Rust's, as Ecto would report them: the message with its
   options, so `errors_on/1`, forms and Gettext work unchanged

The changes the template made that the params did not supply, like the
owner from the scope or the sync time, go to the Rust as data. The other
apps pass them in `data` the same way, and the generated `.rs` says which
fields those are. The template is kept as
`__universal_changeset_template__/4`, and
`test/app/data/universal_changeset_test.exs` runs every case through both
and fails on any difference.

Two generated files exist only while a function is marked:

* `lib/app/data/universal_changeset/native.ex`,
  `App.Data.UniversalChangeset.Native`: `use Rustler`, which compiles the
  NIF crate during `mix compile`, and the NIF and cast list of each
  marked function
* `native/universal_changeset/src/lib.rs`: the NIF crate, including each
  schema's `.rs` by path and exporting one NIF per function

When no function is marked, `mix app.rust` removes both, and nothing in
the repository needs the Rust toolchain: `rustler` is plain Elixir until
a module uses it, and the Dockerfile installs the toolchain only when the
loader exists. The first mark makes cargo a requirement of every
environment that compiles the app. The standalone binary
`mix release app_standalone` builds is not one of them: Burrito cannot
build the NIF for its targets, so it is for apps with no marked function.

## What a marked function may use

The function and every helper it pipes into are pipelines on the
changeset of these calls, with literal arguments. Module attributes such
as `@statuses` resolve to their value. A helper with several clauses is
followed when every clause gives the same Rust, as one that puts a change
or leaves the changeset alone does.

| Ecto | Rust |
| --- | --- |
| `cast/3` | `Changeset::cast`, with Ecto's empty values and type casting |
| `validate_required/3` | `validate_required` |
| `validate_format/4` | `validate_format` with the `regex` crate, flags inline |
| `validate_length/3` with `is`, `min`, `max`, `count` | `validate_length`, graphemes by default |
| `validate_number/3` | `validate_number`, the first failing check reports |
| `validate_inclusion/4`, `exclusion`, `subset` with a list | the same |
| `validate_confirmation/3` | reads `<field>_confirmation` from the params |
| `validate_acceptance/3` | reads the raw param |
| `unique_constraint/3` | `<function>_constraints()`, for the local store to check |
| `put_change/3` | nothing: the app sets the field, and the Rust reads it from `data` |

A put field may be required but not otherwise validated, and may not be
cast too. `unsafe_validate_unique` is noted and not mirrored. A cast
field's type must be one the crate casts: Ecto's built-in types,
`Ecto.UUID` and `Ecto.Enum`. Anything else fails with the file and line:

```
lib/app/data/schemas/users/user.ex:38: email_changeset/3: validate_email/2 must pipe `changeset` through Ecto.Changeset functions
```

Such a function is not marked. Its rules can be split: the part that is a
plain pipeline into a marked function, the rest around it.

Semantics the crate copies from Ecto: a string that is blank once trimmed
is empty, and empty strings are dropped from a list; a validation runs
only on a change that is not null; a field with an error is not reported
missing too; `validate_confirmation` and `validate_acceptance` read the raw
params; errors carry Ecto's options in Ecto's order. Differences to know:

* Ecto compiles regexes with PCRE, the crate with the `regex` crate, which
  has no lookaround or backreferences. Such a pattern does not compile and
  reports as an error on the field, so it is not missed silently
* `:decimal` values compare as `f64`
* a date or time is checked for shape, not for a valid calendar date
* the errors of one `validate_required` call come in field order from Ecto
  and reversed from the Rust, so compare them sorted

## The form, before the cast

A browser form posts `tags[]=vip&tags[]=partner`, and Plug turns that into
`%{"tags" => ["vip", "partner"]}` before any changeset sees it. The crate's
`form` module does the same from a form's `(name, value)` entries, with
`Plug.Conn.Query`'s rules, so the other apps pass their `FormData` entries
straight through and the rules see the params the server would. Its tests
assert what Plug returned for the same entries.

## How it works

* `EctoRust.Source` finds the marked defs in a module's source file, with
  the module's other defs and attributes
* `EctoRust.Transpile` writes the functions as Rust and formats it with
  `rustfmt` when it is installed, so `cargo fmt --check` passes
* `EctoRust.Native` writes the loader module and the NIF crate while a
  function is marked
* `App.Data.UniversalChangeset` is the `use` that captures a marked
  function's template and replaces the function, and `run/3` is what the
  replacement does
* `native/ecto_changeset` is the crate: `changeset` has the Ecto
  semantics, `form` the Plug decoding, and `tests/` holds each to what
  Ecto and Plug return

```sh
mix test test/ecto_rust test/app/data/universal_changeset_test.exs
(cd native/ecto_changeset && cargo test)
```

Known limits: a renamed or removed marked function leaves the NIF crate
naming Rust that no longer exists, and the crate fails to compile before
`mix app.rust` can rewrite it. Delete
`lib/app/data/universal_changeset/native.ex` and run `mix app.rust` again.
`mix app.rust` also compiles the app once after removing the loader,
because Mix would otherwise recreate the removed file empty on the next
compile, see `EctoRust.build/1`.

The spike this came from, the approaches not taken and what each would
take to build are in `docs/design/shared-validations.md`.

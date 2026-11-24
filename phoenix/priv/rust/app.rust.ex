defmodule Mix.Tasks.App.Rust do
  @shortdoc "Writes @universal_changeset changeset functions as Rust"

  @moduledoc """
  Writes every changeset function marked `@universal_changeset true` as Rust
  next to its schema, for the desktop and mobile apps to validate the same
  data the same way before it reaches the server, and for this app to run
  the same Rust. See `priv/rust/README.md`.

      use App.Data.UniversalChangeset

      @universal_changeset true
      def changeset(post, params, options \\\\ []) do
        post
        |> cast(params, [:body])
        |> validate_required([:body])
        |> validate_length(:body, max: 10_000)
      end

  writes `post.rs` in the schema's folder, the function as a chain of calls
  on the `native/ecto_changeset` crate, which the other apps compile in by
  path. While any function is marked it also writes the NIF crate's
  `native/universal_changeset/src/lib.rs` and the module that loads it,
  `lib/app/data/universal_changeset/native.ex`, and removes both when none
  is, so a repository with no mark needs no Rust toolchain.

  ## Usage

      mix app.rust          # write every file that is missing or out of date
      mix app.rust --check  # report files out of date with their functions

  `test/ecto_rust/sync_test.exs` runs the same check as `--check`, so
  `mix test` fails on a file left out of date.
  """

  use Mix.Task

  @switches [check: :boolean]

  @impl Mix.Task
  def run(argv) do
    {opts, _args} = OptionParser.parse!(argv, strict: @switches)
    Mix.Task.run("compile")

    marked = EctoRust.marked(Mix.Project.config()[:app])
    results = if opts[:check], do: EctoRust.check(marked), else: EctoRust.build(marked)
    {problems, done} = Enum.split_with(results, &EctoRust.problem?/1)

    for result <- done, not match?({:unchanged, _}, result), do: Mix.shell().info(EctoRust.describe(result))
    for problem <- problems, do: Mix.shell().error(EctoRust.describe(problem))

    if problems != [] do
      Mix.raise("mix app.rust found #{length(problems)} problem(s)")
    end
  end
end

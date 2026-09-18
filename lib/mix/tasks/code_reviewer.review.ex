defmodule Mix.Tasks.CodeReviewer.Review do
  @shortdoc "Hierarchical text summary of an Elixir code change"

  @moduledoc """
  Summarises a change top-down: files, modules, public and private
  functions, then a per-module tree.

      # one commit in another repository
      mix code_reviewer.review --repo ~/code/form_flow --rev b67155b

      # any two revisions
      mix code_reviewer.review --repo ~/code/form_flow --from v0.3.0 --to main

      # a diff on stdin (best effort: only lines in the diff are known)
      git -C ~/code/form_flow diff HEAD~3 | mix code_reviewer.review --stdin

  Options:

      --repo PATH     repository to read from (default: current directory)
      --rev REV       shorthand for --from REV~1 --to REV
      --from REV      old side
      --to REV        new side (default: HEAD, or the working tree when --from is given alone)
      --stdin         read a unified diff from stdin instead of git
      --parser NAME   auto (default), ast or regex
      --out FILE      write to FILE instead of stdout
  """

  use Mix.Task

  @switches [
    repo: :string,
    rev: :string,
    from: :string,
    to: :string,
    stdin: :boolean,
    parser: :string,
    out: :string
  ]

  @impl Mix.Task
  def run(argv) do
    {opts, _rest, invalid} = OptionParser.parse(argv, strict: @switches)

    if invalid != [], do: Mix.raise("Unknown options: #{inspect(invalid)}")

    review =
      if opts[:stdin] do
        CodeReviewer.review_diff(IO.read(:stdio, :eof), title: opts[:rev])
      else
        {from, to} = revisions(opts)
        repo = opts[:repo] || "."

        case CodeReviewer.review_git(repo, from: from, to: to) do
          {:ok, review} -> review
          {:error, reason} -> Mix.raise(reason)
        end
      end

    text = CodeReviewer.Report.Text.render(review)

    case opts[:out] do
      nil ->
        IO.write(text)

      path ->
        File.mkdir_p!(Path.dirname(path))
        File.write!(path, text)
        Mix.shell().info("Wrote #{path}")
    end
  end

  defp revisions(opts) do
    cond do
      opts[:rev] -> {"#{opts[:rev]}~1", opts[:rev]}
      opts[:from] && opts[:to] -> {opts[:from], opts[:to]}
      opts[:from] -> {opts[:from], "HEAD"}
      true -> {"HEAD~1", "HEAD"}
    end
  end
end

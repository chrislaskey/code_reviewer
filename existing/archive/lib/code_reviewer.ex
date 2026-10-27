defmodule CodeReviewer do
  @moduledoc """
  Hierarchical, deterministic summaries of Elixir code changes.

  GitHub reviews a change one file at a time. `CodeReviewer` reads the same
  change top-down instead: which modules changed, which public and private
  functions inside them, and only then which lines. Outlines come from the
  Elixir AST when a file parses, and from a line-oriented regex pass when it
  does not (partial diffs, broken syntax).

  The typical entry points are:

    * `review_git/2` - compare two revisions of a repository
    * `review_diff/2` - work from unified diff text alone (best effort)
    * `Mix.Tasks.CodeReviewer.Review` - the command line front end
  """

  alias CodeReviewer.{Analysis, Diff, Git, Review}

  @doc """
  Reviews the change between two git revisions of `repo`.

  Full file contents are read from git on both sides, so function ranges and
  unchanged context are exact.
  """
  @spec review_git(Path.t(), keyword()) :: {:ok, Review.t()} | {:error, term()}
  def review_git(repo, opts) do
    from = Keyword.fetch!(opts, :from)
    to = Keyword.fetch!(opts, :to)

    with {:ok, diff_text} <- Git.diff(repo, from, to) do
      files = Diff.parse(diff_text)

      fetch = fn
        _rev, nil -> nil
        rev, path -> Git.show(repo, rev, path) |> ok_or_nil()
      end

      review =
        Analysis.analyze(files,
          source: %{
            kind: :git,
            repo: Path.expand(repo),
            from: from,
            to: to,
            title: Git.subject(repo, to)
          },
          before: fn file -> fetch.(from, file.old_path) end,
          after: fn file -> fetch.(to, file.new_path) end
        )

      {:ok, review}
    end
  end

  @doc """
  Reviews unified diff text with no access to the repository.

  Only lines present in the diff are known, so functions whose definition
  line is outside every hunk cannot be named. Every file is marked with
  `parser: :fragment` so renderers can say so.
  """
  @spec review_diff(String.t(), keyword()) :: Review.t()
  def review_diff(diff_text, opts \\ []) do
    files = Diff.parse(diff_text)

    Analysis.analyze(files,
      source: %{kind: :diff, title: Keyword.get(opts, :title)},
      before: fn _ -> nil end,
      after: fn _ -> nil end
    )
  end

  defp ok_or_nil({:ok, value}), do: value
  defp ok_or_nil(_), do: nil
end

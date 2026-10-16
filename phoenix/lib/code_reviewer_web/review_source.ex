defmodule CodeReviewerWeb.ReviewSource do
  @moduledoc """
  Where a review comes from: a repository path and a revision, taken from the
  URL query string. Shared by every view so `/?repo=…&rev=…` means the same
  thing everywhere.
  """

  alias CodeReviewer.Git

  @type t :: %{repo: Path.t(), rev: String.t()}

  @doc "Reads `repo` and `rev` from URL params, falling back to this repository at `HEAD`."
  @spec from_params(map()) :: t()
  def from_params(params) do
    repo =
      case params["repo"] do
        blank when blank in [nil, ""] -> default_repo()
        repo -> Path.expand(repo)
      end

    rev =
      case params["rev"] do
        blank when blank in [nil, ""] -> "HEAD"
        rev -> rev
      end

    %{repo: repo, rev: rev}
  end

  @doc "The two revisions a review compares: the commit before `rev`, and `rev`."
  @spec revisions(t()) :: %{from: String.t(), to: String.t()}
  def revisions(%{rev: rev}), do: %{from: "#{rev}~1", to: rev}

  @doc "Builds the review for `source`, or explains why it cannot."
  @spec review(t()) :: {:ok, CodeReviewer.Review.t()} | {:error, String.t()}
  def review(%{repo: repo} = source) do
    cond do
      not File.dir?(repo) ->
        {:error, "#{repo} is not a directory"}

      Git.toplevel(repo) == nil ->
        {:error, "#{repo} is not inside a git repository"}

      true ->
        %{from: from, to: to} = revisions(source)

        case CodeReviewer.review_git(repo, from: from, to: to) do
          {:ok, review} -> {:ok, review}
          {:error, reason} -> {:error, to_string(reason)}
        end
    end
  end

  @doc "Contents of `path` at `rev`, or nil when git has nothing for it."
  @spec fetch(Path.t(), String.t(), Path.t()) :: String.t() | nil
  def fetch(repo, rev, path) do
    case Git.show(repo, rev, path) do
      {:ok, text} -> text
      {:error, _} -> nil
    end
  end

  @doc "`~/code/app` instead of the full home path, for labels."
  @spec short_repo(Path.t()) :: String.t()
  def short_repo(path) do
    home = System.user_home!()

    if String.starts_with?(path, home),
      do: "~" <> String.replace_prefix(path, home, ""),
      else: path
  end

  # The repository this app lives in, so the page shows something on first load.
  defp default_repo do
    Git.toplevel(File.cwd!()) || File.cwd!()
  end
end

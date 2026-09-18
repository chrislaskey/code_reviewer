defmodule CodeReviewer.Git do
  @moduledoc "Thin wrapper over the `git` command line for the handful of calls a review needs."

  @doc "Unified diff between two revisions. `--no-color`, no renames guessed beyond git defaults."
  @spec diff(Path.t(), String.t(), String.t()) :: {:ok, String.t()} | {:error, String.t()}
  def diff(repo, from, to) do
    run(repo, ["diff", "--no-color", "--no-ext-diff", "-M", from, to])
  end

  @doc "Contents of `path` at `rev`."
  @spec show(Path.t(), String.t(), Path.t()) :: {:ok, String.t()} | {:error, String.t()}
  def show(repo, rev, path), do: run(repo, ["show", "#{rev}:#{path}"])

  @doc "Subject line of the commit at `rev`, or nil."
  @spec subject(Path.t(), String.t()) :: String.t() | nil
  def subject(repo, rev) do
    case run(repo, ["log", "-1", "--format=%s", rev]) do
      {:ok, out} -> out |> String.trim() |> blank_to_nil()
      _ -> nil
    end
  end

  @doc "Resolves `rev` to a short sha, or nil."
  @spec short_sha(Path.t(), String.t()) :: String.t() | nil
  def short_sha(repo, rev) do
    case run(repo, ["rev-parse", "--short", rev]) do
      {:ok, out} -> out |> String.trim() |> blank_to_nil()
      _ -> nil
    end
  end

  defp run(repo, args) do
    case System.cmd("git", args, cd: Path.expand(repo), stderr_to_stdout: true) do
      {out, 0} -> {:ok, out}
      {out, code} -> {:error, "git #{Enum.join(args, " ")} exited #{code}: #{String.trim(out)}"}
    end
  end

  defp blank_to_nil(""), do: nil
  defp blank_to_nil(s), do: s
end

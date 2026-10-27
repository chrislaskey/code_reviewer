defmodule CodeReviewer.Diff do
  @moduledoc """
  Parses unified diff text as produced by `git diff` and `git show`.

  The parser is deliberately forgiving: anything before the first
  `diff --git` line (commit headers, stat output) is ignored, binary files
  produce a file entry with no hunks, and `\\ No newline at end of file`
  markers are dropped.
  """

  defmodule Hunk do
    @moduledoc "One `@@` hunk. `lines` are `{kind, old_line, new_line, text}` tuples."
    @type kind :: :context | :add | :del
    @type line :: {kind, pos_integer() | nil, pos_integer() | nil, String.t()}
    @type t :: %__MODULE__{}
    defstruct [:old_start, :old_count, :new_start, :new_count, :header, lines: []]
  end

  defmodule File do
    @moduledoc "One `diff --git` section."
    @type t :: %__MODULE__{}
    defstruct [:old_path, :new_path, :status, :similarity, binary: false, hunks: []]
  end

  @type t :: File.t()

  @hunk_header ~r/^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@ ?(.*)$/

  @doc "Parses a full diff into a list of files."
  @spec parse(String.t()) :: [File.t()]
  def parse(text) when is_binary(text) do
    text
    |> String.split(["\r\n", "\n"])
    |> Enum.drop_while(&(not String.starts_with?(&1, "diff --git ")))
    |> parse_files([])
    |> Enum.reverse()
  end

  # -- files -----------------------------------------------------------------

  defp parse_files([], acc), do: acc

  defp parse_files(["diff --git " <> rest | lines], acc) do
    {old_guess, new_guess} = split_git_paths(rest)
    file = %File{old_path: old_guess, new_path: new_guess, status: :updated}
    {file, lines} = parse_headers(lines, file)
    {hunks, lines} = parse_hunks(lines, [])
    file = %{file | hunks: Enum.reverse(hunks)}
    parse_files(lines, [file | acc])
  end

  defp parse_files([_ | lines], acc), do: parse_files(lines, acc)

  # `diff --git a/x b/y` - paths with spaces are rare; split on " b/" from the right.
  defp split_git_paths(rest) do
    case :binary.matches(rest, " b/") do
      [] ->
        {nil, nil}

      matches ->
        {pos, _} = List.last(matches)
        old = rest |> binary_part(0, pos) |> strip_prefix("a/")
        new = rest |> binary_part(pos + 3, byte_size(rest) - pos - 3)
        {old, new}
    end
  end

  defp strip_prefix("a/" <> rest, "a/"), do: rest
  defp strip_prefix(other, _), do: other

  defp parse_headers([line | lines] = all, file) do
    cond do
      String.starts_with?(line, "@@ ") ->
        {file, all}

      String.starts_with?(line, "diff --git ") ->
        {file, all}

      String.starts_with?(line, "new file mode") ->
        parse_headers(lines, %{file | status: :created, old_path: nil})

      String.starts_with?(line, "deleted file mode") ->
        parse_headers(lines, %{file | status: :deleted, new_path: nil})

      String.starts_with?(line, "rename from ") ->
        parse_headers(lines, %{
          file
          | status: :renamed,
            old_path: after_prefix(line, "rename from ")
        })

      String.starts_with?(line, "rename to ") ->
        parse_headers(lines, %{
          file
          | status: :renamed,
            new_path: after_prefix(line, "rename to ")
        })

      String.starts_with?(line, "similarity index ") ->
        sim =
          line
          |> after_prefix("similarity index ")
          |> String.trim_trailing("%")
          |> String.to_integer()

        parse_headers(lines, %{file | similarity: sim})

      String.starts_with?(line, "Binary files ") ->
        {%{file | binary: true}, lines}

      String.starts_with?(line, "--- ") ->
        parse_headers(lines, put_path(file, :old_path, after_prefix(line, "--- ")))

      String.starts_with?(line, "+++ ") ->
        parse_headers(lines, put_path(file, :new_path, after_prefix(line, "+++ ")))

      true ->
        # old mode / new mode / index / copy from ... - not needed
        parse_headers(lines, file)
    end
  end

  defp parse_headers([], file), do: {file, []}

  defp after_prefix(line, prefix),
    do: binary_part(line, byte_size(prefix), byte_size(line) - byte_size(prefix))

  defp put_path(file, _key, "/dev/null"), do: file
  defp put_path(file, key, "a/" <> path), do: Map.put(file, key, strip_tab(path))
  defp put_path(file, key, "b/" <> path), do: Map.put(file, key, strip_tab(path))
  defp put_path(file, key, path), do: Map.put(file, key, strip_tab(path))

  # `--- a/path\t(timestamp)` in some diff flavours
  defp strip_tab(path), do: path |> String.split("\t") |> hd()

  # -- hunks -----------------------------------------------------------------

  defp parse_hunks([line | lines] = all, acc) do
    case Regex.run(@hunk_header, line) do
      [_, os, oc, ns, nc, header] ->
        hunk = %Hunk{
          old_start: String.to_integer(os),
          old_count: count(oc),
          new_start: String.to_integer(ns),
          new_count: count(nc),
          header: String.trim(header)
        }

        {hunk, lines} = parse_hunk_lines(lines, hunk, hunk.old_start, hunk.new_start, [])
        parse_hunks(lines, [hunk | acc])

      nil ->
        if String.starts_with?(line, "diff --git "), do: {acc, all}, else: parse_hunks(lines, acc)
    end
  end

  defp parse_hunks([], acc), do: {acc, []}

  defp count(""), do: 1
  defp count(nil), do: 1
  defp count(str), do: String.to_integer(str)

  defp parse_hunk_lines([line | lines] = all, hunk, old, new, acc) do
    case line do
      " " <> text ->
        parse_hunk_lines(lines, hunk, old + 1, new + 1, [{:context, old, new, text} | acc])

      "-" <> text ->
        parse_hunk_lines(lines, hunk, old + 1, new, [{:del, old, nil, text} | acc])

      "+" <> text ->
        parse_hunk_lines(lines, hunk, old, new + 1, [{:add, nil, new, text} | acc])

      "\\" <> _ ->
        parse_hunk_lines(lines, hunk, old, new, acc)

      # A blank context line may be emitted as an empty string by some tools.
      "" ->
        if done?(hunk, old, new),
          do: {%{hunk | lines: Enum.reverse(acc)}, all},
          else: parse_hunk_lines(lines, hunk, old + 1, new + 1, [{:context, old, new, ""} | acc])

      _ ->
        {%{hunk | lines: Enum.reverse(acc)}, all}
    end
  end

  defp parse_hunk_lines([], hunk, _old, _new, acc), do: {%{hunk | lines: Enum.reverse(acc)}, []}

  defp done?(hunk, old, new),
    do: old >= hunk.old_start + hunk.old_count and new >= hunk.new_start + hunk.new_count

  # -- helpers ---------------------------------------------------------------

  @doc "Counts added and removed lines across all hunks of a file."
  @spec stats(File.t()) :: %{added: non_neg_integer(), removed: non_neg_integer()}
  def stats(%File{hunks: hunks}) do
    Enum.reduce(hunks, %{added: 0, removed: 0}, fn hunk, acc ->
      Enum.reduce(hunk.lines, acc, fn
        {:add, _, _, _}, a -> %{a | added: a.added + 1}
        {:del, _, _, _}, a -> %{a | removed: a.removed + 1}
        _, a -> a
      end)
    end)
  end

  @doc "All changed lines of a file as a flat list of `{kind, old_line, new_line, text}`."
  @spec changed_lines(File.t()) :: [Hunk.line()]
  def changed_lines(%File{hunks: hunks}) do
    for hunk <- hunks, {kind, _, _, _} = line <- hunk.lines, kind != :context, do: line
  end

  @doc """
  Reconstructs the visible fragment of one side of the file from its hunks.

  Returns `{text, line_map}` where `line_map` maps fragment line numbers
  (1-based) back to real line numbers on that side. Lines not present in any
  hunk are absent, so outlines built from fragments are partial by nature.
  """
  @spec fragment(File.t(), :before | :after) :: {String.t(), %{pos_integer() => pos_integer()}}
  def fragment(%File{hunks: hunks}, side) do
    keep = if side == :before, do: [:context, :del], else: [:context, :add]

    lines =
      for hunk <- hunks, {kind, old, new, text} <- hunk.lines, kind in keep do
        {if(side == :before, do: old, else: new), text}
      end

    line_map = lines |> Enum.with_index(1) |> Map.new(fn {{real, _}, idx} -> {idx, real} end)
    {lines |> Enum.map(&elem(&1, 1)) |> Enum.join("\n"), line_map}
  end
end

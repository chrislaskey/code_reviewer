defmodule CodeReviewer.FunctionDiff do
  @moduledoc """
  A whole-function, line-level diff.

  Hunks only carry changed lines and a little context, so to show an entire
  function with its changes marked we rebuild the full two-sided line list
  from the before and after sources plus the hunks, then keep the lines that
  fall inside the function's range on either side.

  Every line is `%{kind: :context | :add | :del, old: n | nil, new: n | nil, text: s}`.
  """

  alias CodeReviewer.{Diff, Review}

  @type line :: %{
          kind: :context | :add | :del,
          old: pos_integer() | nil,
          new: pos_integer() | nil,
          text: String.t()
        }

  @doc """
  Lines of `file` that belong to a function spanning `before_range` on the old
  side and `after_range` on the new side (either may be nil).
  """
  @spec for_ranges(
          Review.File.t(),
          Range.t() | nil,
          Range.t() | nil,
          String.t() | nil,
          String.t() | nil
        ) :: [line()]
  def for_ranges(%Review.File{} = file, before_range, after_range, before_src, after_src) do
    file
    |> full(before_src || "", after_src || "")
    |> Enum.filter(fn line ->
      in_range?(line.new, after_range) or in_range?(line.old, before_range)
    end)
  end

  @doc "The whole file as one two-sided line list."
  @spec full(Review.File.t(), String.t(), String.t()) :: [line()]
  def full(%Review.File{} = file, before_src, after_src) do
    changed = Diff.changed_lines(%Diff.File{hunks: file.hunks})
    dels = for {:del, old, _, text} <- changed, into: %{}, do: {old, text}
    adds = for {:add, _, new, text} <- changed, into: %{}, do: {new, text}
    before = lines(before_src)
    after_ = lines(after_src)

    walk(1, 1, tuple_size(before), tuple_size(after_), before, after_, dels, adds, [])
  end

  # Git lists deletions before additions at the same point, so at each
  # position a deletion wins, then an addition, then a shared context line.
  defp walk(old, new, old_len, new_len, before, after_, dels, adds, acc) do
    cond do
      Map.has_key?(dels, old) ->
        walk(old + 1, new, old_len, new_len, before, after_, dels, adds, [
          %{kind: :del, old: old, new: nil, text: dels[old]} | acc
        ])

      Map.has_key?(adds, new) ->
        walk(old, new + 1, old_len, new_len, before, after_, dels, adds, [
          %{kind: :add, old: nil, new: new, text: adds[new]} | acc
        ])

      old > old_len and new > new_len ->
        Enum.reverse(acc)

      true ->
        text = if new <= new_len, do: elem(after_, new - 1), else: elem(before, old - 1)

        walk(old + 1, new + 1, old_len, new_len, before, after_, dels, adds, [
          %{kind: :context, old: old, new: new, text: text} | acc
        ])
    end
  end

  defp lines(""), do: {}

  defp lines(src) do
    src
    |> String.split(["\r\n", "\n"])
    |> then(fn parts ->
      if String.ends_with?(src, "\n"), do: List.delete_at(parts, -1), else: parts
    end)
    |> List.to_tuple()
  end

  defp in_range?(nil, _range), do: false
  defp in_range?(_n, nil), do: false
  defp in_range?(n, range), do: n in range
end

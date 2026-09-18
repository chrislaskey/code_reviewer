defmodule CodeReviewer.FunctionIndex do
  @moduledoc """
  The flat function index from `ideas-v04.md`: one row per changed function,
  sorted by module then source line. Pure data, so the text renderer and the
  web table both read from it.
  """

  alias CodeReviewer.Review

  @type row :: %{
          id: String.t(),
          verb: Review.verb(),
          visibility: :public | :private,
          kind: atom(),
          module: String.t() | nil,
          function: String.t(),
          name: String.t(),
          arity: non_neg_integer() | nil,
          clauses: %{before: non_neg_integer(), after: non_neg_integer()},
          range: Range.t() | nil,
          side: :before | :after,
          stats: %{added: non_neg_integer(), removed: non_neg_integer()},
          details: [atom()],
          file: String.t(),
          line: pos_integer() | nil
        }

  @doc """
  Every changed function in the review as a flat list of maps.

  Options:

    * `:include_unchanged` - keep `:unchanged` functions too (default false)
    * `:include_tests` - keep ExUnit `test` members (default true)
  """
  @spec rows(Review.t(), keyword()) :: [row()]
  def rows(%Review{files: files}, opts \\ []) do
    include_unchanged? = Keyword.get(opts, :include_unchanged, false)
    include_tests? = Keyword.get(opts, :include_tests, true)

    for file <- files,
        module <- file.modules,
        fun <- module.functions,
        include_unchanged? or fun.verb != :unchanged,
        include_tests? or fun.kind != :test do
      row(file, module, fun)
    end
    |> Enum.sort_by(fn r -> {r.module || "", (r.range && r.range.first) || 0, r.function} end)
  end

  @doc """
  Function rows rolled up under their module, in module order.

  Each entry describes the module (name, verb, file and first line, summed
  stats) and carries its changed functions in `functions`. Modules whose verb
  is not `:unchanged` appear even when none of their functions changed (for
  example only an alias moved), with an empty `functions` list.
  """
  @spec by_module(Review.t(), keyword()) :: [map()]
  def by_module(%Review{files: files} = review, opts \\ []) do
    rows_by_key = review |> rows(opts) |> Enum.group_by(&{&1.file, &1.module})

    for file <- files,
        module <- file.modules,
        functions = Map.get(rows_by_key, {file.path, module.name}, []),
        functions != [] or module.verb != :unchanged do
      range = module.range.after || module.range.before

      %{
        id: module_id(file, module),
        module: module.name,
        verb: module.verb,
        old_name: module.old_name,
        file: file.path,
        line: range && range.first,
        stats: module.stats,
        functions: functions
      }
    end
    |> Enum.sort_by(fn m -> {m.module || "", m.file} end)
  end

  defp module_id(file, module) do
    Base.url_encode64("#{file.path}:#{module.name || "-"}", padding: false)
  end

  @doc "Counts by verb and visibility for a header line."
  @spec summary([row()]) :: map()
  def summary(rows) do
    %{
      total: length(rows),
      by_verb:
        Map.merge(
          %{created: 0, updated: 0, deleted: 0, renamed: 0},
          Enum.frequencies_by(rows, & &1.verb)
        ),
      by_visibility:
        Map.merge(%{public: 0, private: 0}, Enum.frequencies_by(rows, & &1.visibility)),
      added: rows |> Enum.map(& &1.stats.added) |> Enum.sum(),
      removed: rows |> Enum.map(& &1.stats.removed) |> Enum.sum()
    }
  end

  defp row(file, module, fun) do
    {range, side} =
      case fun.range do
        %{after: nil, before: before} -> {before, :before}
        %{after: after_} -> {after_, :after}
      end

    %{
      id: id(file, module, fun),
      verb: fun.verb,
      visibility: fun.visibility,
      kind: fun.kind,
      module: module.name,
      function: signature(fun),
      name: fun.name,
      arity: fun.arity,
      clauses: fun.clauses,
      range: range,
      side: side,
      stats: fun.stats,
      details: fun.details,
      file: file.path,
      line: first_changed_line(file, fun, range, side)
    }
  end

  defp signature(%{kind: :test, name: name}), do: "test #{inspect(name)}"
  defp signature(%{name: name, arity: arity}), do: "#{name}/#{arity}"

  # Stable across renders: file path plus the function identity.
  defp id(file, module, fun) do
    [file.path, module.name || "-", fun.kind, fun.name, fun.arity || "-"]
    |> Enum.join(":")
    |> then(&Base.url_encode64(&1, padding: false))
  end

  # The first changed line inside the function, on the side it exists on, so
  # a UI can jump straight to the edit rather than the def line.
  defp first_changed_line(_file, _fun, nil, _side), do: nil

  defp first_changed_line(file, _fun, range, side) do
    file.hunks
    |> Enum.flat_map(& &1.lines)
    |> Enum.find_value(fn
      {:add, _, new, _} when side == :after -> if new in range, do: new
      {:del, old, _, _} when side == :before -> if old in range, do: old
      _ -> nil
    end)
    |> Kernel.||(range.first)
  end
end

defmodule CodeReviewer.Analysis do
  @moduledoc """
  Turns parsed diff files plus before/after source into a `CodeReviewer.Review`.

  For every file the outline of both sides is computed, functions are matched
  by `{group, name, arity}` and modules by name, and diff hunks are
  attributed to whatever function or directive their line numbers fall in.

  When a side's full source is unavailable (pure diff input) the outline is
  built from the diff fragment instead and line numbers are mapped back to
  the real file. Such files are marked `parser: :fragment`.
  """

  alias CodeReviewer.{Diff, Outline, Review}
  alias CodeReviewer.Review.{Directive, Function, Module}

  @elixir_extensions ~w(.ex .exs)
  @heex_extensions ~w(.heex)

  @doc """
  Options:

    * `:source` - map describing where the change came from
    * `:before` - `fn diff_file -> source | nil end` for the old side
    * `:after`  - `fn diff_file -> source | nil end` for the new side
    * `:parser` - `:auto` (default), `:ast` or `:regex`
  """
  @spec analyze([Diff.File.t()], keyword()) :: Review.t()
  def analyze(diff_files, opts) do
    before_fun = Keyword.fetch!(opts, :before)
    after_fun = Keyword.fetch!(opts, :after)
    parser = Keyword.get(opts, :parser, :auto)

    files =
      diff_files
      |> Enum.map(&analyze_file(&1, before_fun.(&1), after_fun.(&1), parser))
      |> Enum.sort_by(& &1.path)

    %Review{source: Keyword.get(opts, :source, %{}), files: files}
  end

  # -- files -------------------------------------------------------------------

  @doc false
  def analyze_file(%Diff.File{} = diff, before_src, after_src, parser \\ :auto) do
    path = diff.new_path || diff.old_path
    language = language(path)

    file = %Review.File{
      path: path,
      old_path: if(diff.status == :renamed, do: diff.old_path),
      verb: diff.status,
      language: language,
      parser: :none,
      hunks: diff.hunks,
      stats: Diff.stats(diff)
    }

    if language == :elixir and not diff.binary do
      {before_outline, after_outline, used} = outlines(diff, before_src, after_src, parser)
      modules = match_modules(before_outline, after_outline, diff)
      %{file | parser: used, modules: modules}
    else
      file
    end
  end

  defp language(nil), do: :other

  defp language(path) do
    ext = Path.extname(path)

    cond do
      ext in @elixir_extensions -> :elixir
      ext in @heex_extensions -> :heex
      true -> :other
    end
  end

  # Full sources when we have them; diff fragments otherwise.
  defp outlines(diff, before_src, after_src, parser) do
    cond do
      (before_src != nil or diff.status == :created) and
          (after_src != nil or diff.status == :deleted) ->
        b =
          if before_src,
            do: Outline.parse(before_src, parser: parser),
            else: %Outline{parser: :ast, modules: []}

        a =
          if after_src,
            do: Outline.parse(after_src, parser: parser),
            else: %Outline{parser: :ast, modules: []}

        {b.modules, a.modules,
         if(b.parser == :regex or a.parser == :regex, do: :regex, else: :ast)}

      true ->
        {b_text, b_map} = Diff.fragment(diff, :before)
        {a_text, a_map} = Diff.fragment(diff, :after)
        name = header_module_name(diff)

        b =
          Outline.parse(b_text, parser: :regex).modules
          |> Outline.remap_lines(&Map.get(b_map, &1, &1))
          |> name_orphan(name)

        a =
          Outline.parse(a_text, parser: :regex).modules
          |> Outline.remap_lines(&Map.get(a_map, &1, &1))
          |> name_orphan(name)

        {b, a, :fragment}
    end
  end

  # git puts the nearest preceding line that starts at column 0 in the hunk
  # header, which for Elixir is almost always `defmodule Name do`.
  @header_module_re ~r/defmodule\s+([A-Z][\w.]*)/

  defp header_module_name(%Diff.File{hunks: hunks}) do
    Enum.find_value(hunks, fn hunk ->
      case Regex.run(@header_module_re, hunk.header || "") do
        [_, name] -> name
        nil -> nil
      end
    end)
  end

  defp name_orphan(modules, nil), do: modules

  defp name_orphan(modules, name),
    do: Enum.map(modules, fn m -> if m.name == nil, do: %{m | name: name}, else: m end)

  # -- modules -------------------------------------------------------------------

  defp match_modules(before_mods, after_mods, diff) do
    before_by_name = Map.new(before_mods, &{&1.name, &1})
    after_by_name = Map.new(after_mods, &{&1.name, &1})

    {removed, added} = {
      Enum.reject(before_mods, &Map.has_key?(after_by_name, &1.name)),
      Enum.reject(after_mods, &Map.has_key?(before_by_name, &1.name))
    }

    # One module gone and one new in the same file is a rename.
    renames =
      case {removed, added} do
        {[old], [new]} -> [{old, new}]
        _ -> []
      end

    renamed_old = Enum.map(renames, fn {old, _} -> old.name end)
    renamed_new = Enum.map(renames, fn {_, new} -> new.name end)

    kept =
      for a <- after_mods, b = before_by_name[a.name], do: build_module(b, a, :updated, diff)

    created = for a <- added, a.name not in renamed_new, do: build_module(nil, a, :created, diff)

    deleted =
      for b <- removed, b.name not in renamed_old, do: build_module(b, nil, :deleted, diff)

    renamed = for {b, a} <- renames, do: %{build_module(b, a, :renamed, diff) | old_name: b.name}

    (kept ++ created ++ deleted ++ renamed)
    |> Enum.map(&settle_module_verb/1)
    |> Enum.sort_by(fn m -> {m.range.after || m.range.before || 0, m.name || ""} end)
  end

  defp build_module(before, after_, verb, diff) do
    functions = match_functions(before, after_, diff)
    directives = match_directives(before, after_, diff)

    %Module{
      name: (after_ || before).name,
      verb: verb,
      range: %{before: range(before), after: range(after_)},
      functions: functions,
      directives: directives,
      stats: module_stats(before, after_, diff)
    }
  end

  # An "updated" module where nothing inside moved is unchanged (a hunk may
  # touch only whitespace or comments in it).
  defp settle_module_verb(%Module{verb: :updated} = m) do
    touched? =
      Enum.any?(m.functions, &(&1.verb != :unchanged)) or
        Enum.any?(m.directives, &(&1.verb != :unchanged)) or
        m.stats.added + m.stats.removed > 0

    if touched?, do: m, else: %{m | verb: :unchanged}
  end

  defp settle_module_verb(m), do: m

  defp range(nil), do: nil
  defp range(%{line_start: s, line_end: e}), do: s..e

  defp module_stats(before, after_, diff) do
    Enum.reduce(Diff.changed_lines(diff), %{added: 0, removed: 0}, fn
      {:add, _, new, _}, acc ->
        if after_ && in_range?(new, range(after_)), do: %{acc | added: acc.added + 1}, else: acc

      {:del, old, _, _}, acc ->
        if before && in_range?(old, range(before)),
          do: %{acc | removed: acc.removed + 1},
          else: acc
    end)
  end

  # -- functions -------------------------------------------------------------------

  defp match_functions(before, after_, diff) do
    b_funs = if before, do: before.functions, else: []
    a_funs = if after_, do: after_.functions, else: []

    key = fn f -> {Outline.group(f.kind), f.name, f.arity} end
    b_map = Map.new(b_funs, &{key.(&1), &1})
    a_map = Map.new(a_funs, &{key.(&1), &1})

    kept = for a <- a_funs, b = b_map[key.(a)], do: build_function(b, a, diff)
    created = for a <- a_funs, not Map.has_key?(b_map, key.(a)), do: build_function(nil, a, diff)
    deleted = for b <- b_funs, not Map.has_key?(a_map, key.(b)), do: build_function(b, nil, diff)

    (kept ++ created ++ deleted)
    |> Enum.sort_by(fn f ->
      {f.range.after && f.range.after.first, f.range.before && f.range.before.first}
    end)
  end

  defp build_function(before, after_, diff) do
    stats = %{
      added: count_lines(diff, :add, after_ && range(after_)),
      removed: count_lines(diff, :del, before && range(before))
    }

    {verb, details} = function_verb(before, after_, stats)
    f = after_ || before

    %Function{
      name: f.name,
      arity: f.arity,
      visibility: f.visibility,
      kind: f.kind,
      verb: verb,
      details: details,
      range: %{before: range(before), after: range(after_)},
      clauses: %{before: clause_count(before), after: clause_count(after_)},
      heads: %{before: heads(before), after: heads(after_)},
      annotations: %{before: annotations(before), after: annotations(after_)},
      stats: stats
    }
  end

  defp function_verb(nil, _after, _stats), do: {:created, []}
  defp function_verb(_before, nil, _stats), do: {:deleted, []}

  defp function_verb(before, after_, stats) do
    details =
      []
      |> maybe(before.visibility != after_.visibility, :visibility)
      |> maybe(length(before.clauses) != length(after_.clauses), :clauses)
      |> maybe(heads(before) != heads(after_), :head)
      |> maybe(annotation_change(before, after_, :"@spec"), :spec)
      |> maybe(annotation_change(before, after_, :"@doc"), :doc)
      |> maybe(annotation_change(before, after_, :"@impl"), :impl)
      |> maybe(stats.added + stats.removed > 0, :body)
      |> Enum.reverse()

    if details == [], do: {:unchanged, []}, else: {:updated, details}
  end

  defp maybe(list, true, tag), do: [tag | list]
  defp maybe(list, false, _tag), do: list

  defp annotation_change(before, after_, kind) do
    pick = fn f -> f.annotations |> Enum.filter(&(&1.kind == kind)) |> Enum.map(& &1.text) end
    pick.(before) != pick.(after_)
  end

  defp clause_count(nil), do: 0
  defp clause_count(f), do: length(f.clauses)

  defp heads(nil), do: []
  defp heads(f), do: Enum.map(f.clauses, & &1.head)

  defp annotations(nil), do: []
  defp annotations(f), do: Enum.map(f.annotations, &%{kind: &1.kind, text: &1.text})

  defp count_lines(_diff, _kind, nil), do: 0

  defp count_lines(diff, kind, range) do
    diff
    |> Diff.changed_lines()
    |> Enum.count(fn
      {:add, _, new, _} when kind == :add -> in_range?(new, range)
      {:del, old, _, _} when kind == :del -> in_range?(old, range)
      _ -> false
    end)
  end

  defp in_range?(_n, nil), do: false
  defp in_range?(n, range), do: n in range

  # -- directives --------------------------------------------------------------------

  defp match_directives(before, after_, diff) do
    b = if before, do: before.directives, else: []
    a = if after_, do: after_.directives, else: []

    key = fn d -> {d.kind, d.text} end
    b_keys = MapSet.new(b, key)
    a_keys = MapSet.new(a, key)

    created = for d <- a, not MapSet.member?(b_keys, key.(d)), do: directive(d, nil, :created)
    deleted = for d <- b, not MapSet.member?(a_keys, key.(d)), do: directive(nil, d, :deleted)

    # A one-line directive whose text changed shows up as delete + create; fold
    # same-kind singletons (@moduledoc, @behaviour, defstruct) into :updated.
    {created, deleted} = fold_singletons(created, deleted)

    unchanged =
      for d <- a, MapSet.member?(b_keys, key.(d)) do
        old = Enum.find(b, &(key.(&1) == key.(d)))
        verb = if touched?(diff, d.line, old.line), do: :updated, else: :unchanged
        directive(d, old, verb)
      end

    (created ++ deleted ++ unchanged)
    |> Enum.sort_by(fn d -> {d.line.after || d.line.before || 0, d.text} end)
  end

  @singleton_kinds ~w(moduledoc behaviour defstruct defexception)a

  defp fold_singletons(created, deleted) do
    Enum.reduce(@singleton_kinds, {created, deleted}, fn kind, {c, d} ->
      case {Enum.filter(c, &(&1.kind == kind)), Enum.filter(d, &(&1.kind == kind))} do
        {[new], [old]} ->
          merged = %{
            new
            | verb: :updated,
              line: %{before: old.line.before, after: new.line.after}
          }

          {[merged | Enum.reject(c, &(&1.kind == kind))], Enum.reject(d, &(&1.kind == kind))}

        _ ->
          {c, d}
      end
    end)
  end

  defp directive(new, old, verb) do
    d = new || old

    %Directive{
      kind: d.kind,
      text: d.text,
      verb: verb,
      line: %{before: old && old.line, after: new && new.line}
    }
  end

  # A multi-line directive (attr with doc) is touched when any changed line
  # sits on its first line; good enough for a first pass.
  defp touched?(diff, new_line, old_line) do
    Enum.any?(Diff.changed_lines(diff), fn
      {:add, _, n, _} -> n == new_line
      {:del, o, _, _} -> o == old_line
    end)
  end
end

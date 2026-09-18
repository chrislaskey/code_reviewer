defmodule CodeReviewer.Report.Text do
  @moduledoc """
  Renders a `CodeReviewer.Review` as flat, greppable text.

  Sections follow `ideas-v02.md` (Files, Modules, Public Functions, Private
  Functions) and are followed by a per-module tree in the spirit of
  `ideas-v03.md`. Every renderer here is a pure function from the review to
  a string, so alternative formats can be added beside it.
  """

  alias CodeReviewer.Review

  @verbs [:created, :deleted, :updated, :renamed]

  @doc "The whole report: summary, files, modules, functions, per-module detail."
  @spec render(Review.t()) :: String.t()
  def render(%Review{} = review) do
    [
      summary(review),
      files(review),
      modules(review),
      functions(review, :public),
      functions(review, :private),
      directives(review),
      tree(review)
    ]
    |> Enum.reject(&(&1 == ""))
    |> Enum.join("\n\n")
    |> Kernel.<>("\n")
  end

  # -- summary ---------------------------------------------------------------------

  @doc false
  def summary(%Review{source: source, files: files}) do
    title =
      case source do
        %{kind: :git, from: from, to: to} -> "Change #{to} (from #{from})#{title_suffix(source)}"
        %{title: t} when is_binary(t) -> "Change: #{t}"
        _ -> "Change"
      end

    stats =
      Enum.reduce(
        files,
        %{added: 0, removed: 0},
        &%{added: &2.added + &1.stats.added, removed: &2.removed + &1.stats.removed}
      )

    all_mods = for f <- files, m <- f.modules, do: m
    all_funs = for m <- all_mods, fun <- m.functions, do: fun
    changed_funs = Enum.reject(all_funs, &(&1.verb == :unchanged))
    changed_mods = Enum.reject(all_mods, &(&1.verb == :unchanged))
    parsers = files |> Enum.map(& &1.parser) |> Enum.reject(&(&1 == :none)) |> Enum.frequencies()

    [
      title,
      "#{length(files)} files  +#{stats.added} -#{stats.removed}   #{Enum.count(files, &(&1.language == :elixir))} elixir",
      "Modules   #{length(changed_mods)} changed   " <> verb_counts(changed_mods),
      "Functions #{length(changed_funs)} changed   " <>
        verb_counts(changed_funs) <>
        "   public #{Enum.count(changed_funs, &(&1.visibility == :public))}  private #{Enum.count(changed_funs, &(&1.visibility == :private))}",
      "Parsers   " <>
        Enum.map_join(parsers, "  ", fn {k, v} -> "#{k} #{v}" end) <> parser_note(parsers)
    ]
    |> Enum.join("\n")
  end

  defp title_suffix(%{title: t}) when is_binary(t), do: "  #{t}"
  defp title_suffix(_), do: ""

  defp verb_counts(items) do
    Enum.map_join(@verbs, "  ", fn v -> "#{v} #{Enum.count(items, &(&1.verb == v))}" end)
  end

  defp parser_note(%{fragment: _}), do: "   (fragment: only lines present in the diff are known)"
  defp parser_note(_), do: ""

  # -- files ------------------------------------------------------------------------

  @doc false
  def files(%Review{files: files}) do
    section("Files", @verbs, files, fn f ->
      case f.verb do
        :renamed -> " - #{f.old_path} -> #{f.path}"
        _ -> " - #{f.path}"
      end
    end)
  end

  # -- modules ------------------------------------------------------------------------

  @doc false
  def modules(%Review{files: files}) do
    rows = for f <- files, m <- f.modules, m.verb != :unchanged, do: {m, f}

    width =
      rows |> Enum.map(fn {m, _} -> String.length(m.name || "?") end) |> Enum.max(fn -> 0 end)

    section(
      "Modules",
      @verbs,
      rows,
      fn {m, f} ->
        name =
          if m.verb == :renamed,
            do: "#{m.old_name} -> #{m.name}",
            else: m.name || "(no defmodule in fragment)"

        " - #{String.pad_trailing(name, width)}  #{f.path}"
      end,
      fn {m, _} -> m.verb end
    )
  end

  # -- functions --------------------------------------------------------------------

  @doc false
  def functions(%Review{files: files}, visibility) do
    groups =
      for f <- files, m <- f.modules do
        funs = Enum.filter(m.functions, &(&1.visibility == visibility and &1.verb != :unchanged))
        {m, f, funs}
      end
      |> Enum.reject(fn {_, _, funs} -> funs == [] end)

    heading = "## #{if visibility == :public, do: "Public", else: "Private"} Functions"

    if groups == [] do
      heading <> "\n\n(none)"
    else
      body =
        Enum.map_join(groups, "\n\n", fn {m, f, funs} ->
          width = funs |> Enum.map(&String.length(sig(&1))) |> Enum.max()

          [
            "#{m.name || "(no defmodule in fragment)"} (#{cap(m.verb)})  #{f.path}"
            | Enum.map(funs, fn fun ->
                " - #{cap(fun.verb) |> String.pad_trailing(7)} #{String.pad_trailing(sig(fun), width)}  #{fun_meta(fun)}"
              end)
          ]
          |> Enum.join("\n")
        end)

      heading <> "\n\n" <> body
    end
  end

  defp sig(%{kind: :test, name: name}), do: "test #{inspect(name)}"
  defp sig(%{name: name, arity: arity}), do: "#{name}/#{arity}"

  defp fun_meta(fun) do
    parts = [
      stats(fun.stats),
      lines(fun),
      clauses(fun),
      details(fun)
    ]

    parts |> Enum.reject(&(&1 in [nil, ""])) |> Enum.join("  ")
  end

  defp stats(%{added: a, removed: r}), do: "+#{a} -#{r}"

  defp lines(%{range: %{after: nil, before: r}}) when not is_nil(r),
    do: "was :#{r.first}-#{r.last}"

  defp lines(%{range: %{after: r}}) when not is_nil(r), do: ":#{r.first}-#{r.last}"
  defp lines(_), do: nil

  defp clauses(%{clauses: %{before: b, after: a}}) when b != a and b > 0 and a > 0,
    do: "clauses #{b}->#{a}"

  defp clauses(%{clauses: %{after: a}}) when a > 1, do: "clauses #{a}"
  defp clauses(_), do: nil

  defp details(%{details: []}), do: nil
  defp details(%{details: d}), do: "[" <> Enum.map_join(d, ", ", &Atom.to_string/1) <> "]"

  # -- directives -------------------------------------------------------------------

  @doc false
  def directives(%Review{files: files}) do
    groups =
      for f <- files, m <- f.modules do
        {m, Enum.reject(m.directives, &(&1.verb == :unchanged))}
      end
      |> Enum.reject(fn {_, ds} -> ds == [] end)

    if groups == [] do
      ""
    else
      body =
        Enum.map_join(groups, "\n\n", fn {m, ds} ->
          [
            m.name || "(no defmodule in fragment)"
            | Enum.map(ds, &" #{mark(&1.verb)} #{&1.kind} #{&1.text}")
          ]
          |> Enum.join("\n")
        end)

      "## Directives (alias / import / use / attr / @moduledoc ...)\n\n" <> body
    end
  end

  defp mark(:created), do: "+"
  defp mark(:deleted), do: "-"
  defp mark(:updated), do: "~"
  defp mark(_), do: " "

  # -- per-module tree ---------------------------------------------------------------

  @doc false
  def tree(%Review{files: files}) do
    body =
      files
      |> Enum.filter(&(&1.language == :elixir))
      |> Enum.map_join("\n\n", fn f ->
        header =
          "#{f.path}  #{cap(f.verb)}  +#{f.stats.added} -#{f.stats.removed}#{parser_tag(f.parser)}"

        mods =
          Enum.map_join(f.modules, "\n", fn m ->
            {changed_ds, quiet_ds} = Enum.split_with(m.directives, &(&1.verb != :unchanged))
            {changed_fs, quiet_fs} = Enum.split_with(m.functions, &(&1.verb != :unchanged))

            rows =
              Enum.map(changed_ds, fn d ->
                {d.line.after || d.line.before || 0, tree_directive(d)}
              end) ++
                Enum.map(changed_fs, fn fun ->
                  {(fun.range.after || fun.range.before).first, tree_function(fun)}
                end)

            quiet =
              case {quiet_ds, quiet_fs} do
                {[], []} ->
                  []

                _ ->
                  [
                    "      (unchanged: #{length(quiet_fs)} functions, #{length(quiet_ds)} directives)"
                  ]
              end

            [
              "  #{m.name || "(no defmodule in fragment)"}  #{cap(m.verb)}  +#{m.stats.added} -#{m.stats.removed}"
              | (rows |> Enum.sort_by(&elem(&1, 0)) |> Enum.map(&elem(&1, 1))) ++ quiet
            ]
            |> Enum.join("\n")
          end)

        if mods == "", do: header, else: header <> "\n" <> mods
      end)

    if body == "", do: "", else: "## Tree\n\n" <> body
  end

  defp parser_tag(:fragment), do: "  (fragment)"
  defp parser_tag(_), do: ""

  defp tree_directive(d), do: "    #{mark(d.verb)} #{d.kind} #{d.text}"

  defp tree_function(fun) do
    label = if fun.kind == :test, do: sig(fun), else: "#{fun.kind} #{sig(fun)}"
    base = "    #{mark(fun.verb)} #{label}  #{cap(fun.verb)}  #{fun_meta(fun)}"

    clause_rows =
      if fun.kind != :test and (:clauses in fun.details or fun.verb == :created) do
        Enum.map(fun.heads.after, &"        #{&1}")
      else
        []
      end

    Enum.join([base | clause_rows], "\n")
  end

  # -- helpers ----------------------------------------------------------------------

  defp section(title, verbs, items, render, verb_of \\ & &1.verb) do
    blocks =
      for verb <- verbs, rows = Enum.filter(items, &(verb_of.(&1) == verb)), rows != [] do
        "#{cap(verb)}:\n" <> Enum.map_join(rows, "\n", render)
      end

    "## #{title}\n\n" <> if(blocks == [], do: "(none)", else: Enum.join(blocks, "\n\n"))
  end

  defp cap(atom), do: atom |> Atom.to_string() |> String.capitalize()
end

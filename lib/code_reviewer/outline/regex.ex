defmodule CodeReviewer.Outline.Regex do
  @moduledoc """
  Line-oriented outline parser. No AST, no compiler: indentation and a
  handful of regular expressions.

  Assumptions, all true for `mix format`ed code and mostly true otherwise:

    * a `defmodule` ends at the first later `end` with the same indentation
    * a function clause ends just before the first later non-blank line
      whose indentation is not deeper than its own (or at that line, when it
      is the matching `end`)
    * multi-line heads keep going until parentheses balance and a `do` or
      `do:` appears
    * heredocs (`\"\"\"`, `'''`) and `#` comment lines are skipped

  Works on fragments: anything that does not match is ignored rather than
  raising, so a diff hunk with no `defmodule` still yields functions under a
  module named `nil`.
  """

  alias CodeReviewer.Outline
  alias CodeReviewer.Outline.{Directive, Module}

  @definers Outline.definers() |> Enum.map(&Atom.to_string/1)
  @def_re ~r/^(?<indent>\s*)(?<kind>#{Enum.join(@definers, "|")})\s+(?<rest>.*)$/
  @module_re ~r/^(?<indent>\s*)defmodule\s+(?<name>(?:__MODULE__|[A-Z])[\w.]*)\s+do\b/
  @end_re ~r/^(?<indent>\s*)end\b/
  @block_continuation_re ~r/^\s*(rescue|catch|after|else)\b/
  @directive_re ~r/^\s*(?<kind>alias|import|use|require|attr|slot|@moduledoc|@behaviour|@derive|defstruct|defexception|@type|@typep|@opaque|@callback|@macrocallback)\b\s*(?<rest>.*)$/
  @annotation_re ~r/^\s*(?<kind>@doc|@spec|@impl|@deprecated)\b\s*(?<rest>.*)$/
  @name_re ~r/^(?<name>[a-z_][\w]*[?!]?|unquote\([^)]*\))/
  @test_re ~r/^(?<indent>\s*)test\s+(?<name>"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')/

  @doc "Parses `source` into a list of `CodeReviewer.Outline.Module` structs."
  @spec parse(String.t()) :: [Module.t()]
  def parse(source) when is_binary(source) do
    lines =
      source
      |> String.split(["\r\n", "\n"])
      |> Enum.with_index(1)
      |> mark_skipped()

    scan(lines, lines, %{stack: [], done: [], pending: []})
  end

  # -- pre-pass: heredocs and comments ---------------------------------------
  #
  # Produces `{text, line_no, skip?}` so the main scan never looks inside a
  # heredoc or at a comment line.

  defp mark_skipped(lines) do
    {out, _} =
      Enum.map_reduce(lines, nil, fn {text, no}, heredoc ->
        trimmed = String.trim(text)

        cond do
          heredoc != nil ->
            closing? = String.starts_with?(trimmed, heredoc)
            {{text, no, true}, if(closing?, do: nil, else: heredoc)}

          String.starts_with?(trimmed, "#") ->
            {{text, no, true}, nil}

          true ->
            case opens_heredoc(text) do
              nil -> {{text, no, false}, nil}
              delim -> {{text, no, false}, delim}
            end
        end
      end)

    out
  end

  # A heredoc opens when a line ends (modulo whitespace) with """ or '''.
  # Lines that both open and close (`x = """ ... """`) are not heredocs.
  defp opens_heredoc(text) do
    trimmed = String.trim_trailing(text)

    cond do
      String.ends_with?(trimmed, "\"\"\"") and count(trimmed, "\"\"\"") == 1 -> "\"\"\""
      String.ends_with?(trimmed, "'''") and count(trimmed, "'''") == 1 -> "'''"
      true -> nil
    end
  end

  defp count(text, needle), do: text |> :binary.matches(needle) |> length()

  # -- main scan -------------------------------------------------------------
  #
  # state.stack   open modules (innermost first), each with its clauses so far
  # state.done    closed modules
  # state.pending annotations (@doc/@spec/@impl) waiting for the next def
  # `all` is the full line list, needed to look ahead for block ends.

  defp scan([], all, state) do
    # Unterminated modules (fragments) close at the last line.
    last = all |> List.last() |> line_no()
    state = Enum.reduce(state.stack, state, fn _m, st -> close_module(st, last) end)

    state.done
    |> Enum.sort_by(& &1.line_start)
    |> maybe_orphan_module(state, all)
  end

  defp scan([{_text, _no, true} | rest], all, state), do: scan(rest, all, state)

  defp scan([{text, no, false} | rest], all, state) do
    cond do
      captures = Regex.named_captures(@module_re, text) ->
        indent = String.length(captures["indent"])
        line_end = find_block_end(all, no, indent)
        name = qualify(captures["name"], state.stack)

        mod = %{
          name: name,
          indent: indent,
          line_start: no,
          line_end: line_end,
          clauses: [],
          directives: []
        }

        scan(rest, all, %{state | stack: [mod | state.stack], pending: []})

      Regex.match?(@end_re, text) and closes_top_module?(state, text) ->
        scan(rest, all, close_module(state, no))

      captures = Regex.named_captures(@def_re, text) ->
        indent = String.length(captures["indent"])
        {head, head_end} = collect_head(captures["rest"], rest)
        head_end = head_end || no
        line_end = clause_end(all, head_end, indent)
        clause = build_clause(captures["kind"], head, no, line_end, state.pending)
        state = push_clause(%{state | pending: []}, clause)
        # skip nested defs (quote blocks, defs inside defs)
        scan(drop_until(rest, line_end), all, state)

      captures = Regex.named_captures(@test_re, text) ->
        indent = String.length(captures["indent"])
        line_end = clause_end(all, no, indent)
        name = captures["name"] |> String.slice(1..-2//1) |> unescape()

        clause = %{
          kind: :test,
          name: name,
          arity: nil,
          head: "test #{captures["name"]}",
          line_start: no,
          line_end: line_end,
          annotations: state.pending
        }

        state = push_clause(%{state | pending: []}, clause)
        scan(drop_until(rest, line_end), all, state)

      captures = Regex.named_captures(@annotation_re, text) ->
        ann = %{
          kind: String.to_atom(captures["kind"]),
          text: String.trim(captures["rest"]),
          line: no
        }

        scan(rest, all, %{state | pending: state.pending ++ [ann]})

      captures = Regex.named_captures(@directive_re, text) ->
        state = push_directive(state, captures["kind"], captures["rest"], no)
        scan(rest, all, state)

      true ->
        scan(rest, all, state)
    end
  end

  # -- modules ---------------------------------------------------------------

  # Nested `defmodule Bar` inside `Foo` is `Foo.Bar`; `__MODULE__.Bar` likewise.
  defp qualify(name, []), do: name
  defp qualify("__MODULE__" <> rest, [parent | _]), do: parent.name <> rest
  defp qualify(name, [parent | _]) when is_binary(parent.name), do: parent.name <> "." <> name
  defp qualify(name, _stack), do: name

  defp closes_top_module?(%{stack: [top | _]}, text) do
    indent = @end_re |> Regex.named_captures(text) |> Map.fetch!("indent") |> String.length()
    indent == top.indent
  end

  defp closes_top_module?(_state, _text), do: false

  defp close_module(%{stack: [top | rest]} = state, line_no) do
    module = %Module{
      name: top.name,
      line_start: top.line_start,
      line_end: line_no,
      functions: top.clauses |> Enum.reverse() |> Outline.group_clauses(),
      directives: Enum.reverse(top.directives)
    }

    %{state | stack: rest, done: [module | state.done]}
  end

  defp close_module(state, _line_no), do: state

  # Fragments may define functions before any `defmodule`. Keep them under a
  # nameless module so nothing is silently dropped.
  defp maybe_orphan_module(modules, %{orphans: clauses}, all)
       when is_list(clauses) and clauses != [] do
    first = all |> hd() |> line_no()
    last = all |> List.last() |> line_no()

    orphan = %Module{
      name: nil,
      line_start: first,
      line_end: last,
      functions: clauses |> Enum.reverse() |> Outline.group_clauses(),
      directives: []
    }

    [orphan | modules]
  end

  defp maybe_orphan_module(modules, _state, _all), do: modules

  defp push_clause(%{stack: [top | rest]} = state, clause),
    do: %{state | stack: [%{top | clauses: [clause | top.clauses]} | rest]}

  defp push_clause(state, clause),
    do: Map.update(state, :orphans, [clause], &[clause | &1])

  defp push_directive(%{stack: [top | rest]} = state, kind, rest_text, no) do
    directive = %Directive{
      kind: directive_kind(kind),
      text: directive_text(kind, rest_text),
      line: no
    }

    %{state | stack: [%{top | directives: [directive | top.directives]} | rest]}
  end

  defp push_directive(state, _kind, _rest, _no), do: state

  defp directive_kind("@" <> rest), do: String.to_atom(rest)
  defp directive_kind(kind), do: String.to_atom(kind)

  # `alias Foo.Bar, as: Baz` -> "Foo.Bar, as: Baz"; `attr(:name, :string, ...)` -> ":name, :string, ..."
  defp directive_text(_kind, rest) do
    rest
    |> String.trim()
    |> String.replace_leading("(", "")
    |> String.trim_trailing(")")
    |> String.trim_trailing(",")
    |> String.trim()
  end

  # -- function heads ----------------------------------------------------------
  #
  # A head runs from after the `def` keyword until a `do` block opener or a
  # `do:` keyword at paren depth zero. Formatted code can split a head across
  # lines after a comma or before `when`, so we keep appending lines until
  # depth is zero and a `do` is seen (or the line ends without a trailing
  # comma / `when`, which means a bodiless head).

  defp collect_head(first, rest) do
    do_collect_head(first, rest, nil)
  end

  defp do_collect_head(acc, rest, last_no) do
    stripped = strip_do(acc)

    cond do
      stripped != acc ->
        {String.trim(stripped), last_no}

      continues?(acc) ->
        case Enum.find(rest, fn {_t, _n, skip} -> not skip end) do
          nil ->
            {String.trim(acc), last_no}

          {text, no, _} ->
            rest = Enum.drop_while(rest, fn {_t, n, _} -> n <= no end)
            do_collect_head(acc <> " " <> String.trim(text), rest, no)
        end

      true ->
        {String.trim(acc), last_no}
    end
  end

  # Remove a trailing ` do` / `, do: ...` at depth zero, returning the head only.
  defp strip_do(text) do
    cond do
      Regex.match?(~r/\sdo\s*$/, text) and depth(text) == 0 ->
        Regex.replace(~r/\sdo\s*$/, text, "")

      Regex.match?(~r/^do\s*$/, text) ->
        ""

      true ->
        case do_keyword_at_depth_zero(text) do
          nil -> text
          idx -> binary_part(text, 0, idx)
        end
    end
  end

  # Find `, do:` (or bare `do:` for `def foo, do:`) outside any brackets.
  defp do_keyword_at_depth_zero(text) do
    Regex.scan(~r/,\s*do:/, text, return: :index)
    |> Enum.map(fn [{idx, _}] -> idx end)
    |> Enum.find(fn idx -> depth(binary_part(text, 0, idx)) == 0 end)
  end

  defp continues?(text) do
    trimmed = String.trim_trailing(text)

    depth(trimmed) > 0 or String.ends_with?(trimmed, ",") or String.ends_with?(trimmed, " when") or
      String.ends_with?(trimmed, "when") or
      Regex.match?(~r/\b(and|or|in|not|\|\||&&|==|!=|<|>|<=|>=|\+|-|\*|\/)\s*$/, trimmed)
  end

  # Bracket depth, ignoring brackets inside strings and charlists (approximate).
  defp depth(text) do
    text
    |> strip_strings()
    |> String.graphemes()
    |> Enum.reduce(0, fn
      c, d when c in ["(", "[", "{"] -> d + 1
      c, d when c in [")", "]", "}"] -> d - 1
      _, d -> d
    end)
  end

  # Char literals (`?a`, `?(`) go first so `?"` cannot open a string; a `?`
  # right after a word character is part of a name such as `valid?(`.
  defp strip_strings(text) do
    text
    |> then(&Regex.replace(~r/(?<![\w?!])\?./, &1, ""))
    |> then(&Regex.replace(~r/"(?:[^"\\]|\\.)*"/, &1, "\"\""))
    |> then(&Regex.replace(~r/'(?:[^'\\]|\\.)*'/, &1, "''"))
  end

  defp build_clause(kind, head, line_start, line_end, pending) do
    {name, args} = split_name_args(head)

    %{
      kind: String.to_atom(kind),
      name: name,
      arity: arity(args),
      head: head,
      line_start: line_start,
      line_end: line_end,
      annotations: pending
    }
  end

  # "foo(a, b) when a > 1" -> {"foo", "a, b"};  "foo" -> {"foo", ""};  "a <~> b" -> {"<~>", "a, b"}
  defp split_name_args(head) do
    head = head |> String.trim() |> strip_guard()

    case Regex.run(@name_re, head, capture: :first) do
      [name] ->
        rest = head |> String.replace_prefix(name, "") |> String.trim()

        cond do
          String.starts_with?(rest, "(") -> {name, inner_parens(rest)}
          rest == "" -> {name, ""}
          # `def foo a, b` without parens (rare)
          true -> {name, rest}
        end

      nil ->
        # operator definitions: `def a + b`, `defmacro left ~> right`
        case Regex.run(~r/^(\S+)\s+(\S+)\s+(\S+)$/, head) do
          [_, l, op, r] -> {op, "#{l}, #{r}"}
          _ -> {head, ""}
        end
    end
  end

  defp strip_guard(head) do
    case Regex.split(~r/\swhen\s/, head, parts: 2) do
      [before, _guard] -> before
      [only] -> only
    end
  end

  # Text inside the first balanced pair of parentheses.
  defp inner_parens("(" <> rest) do
    rest
    |> String.graphemes()
    |> Enum.reduce_while({1, []}, fn
      ")", {1, acc} -> {:halt, {0, acc}}
      ")", {d, acc} -> {:cont, {d - 1, [")" | acc]}}
      "(", {d, acc} -> {:cont, {d + 1, ["(" | acc]}}
      c, {d, acc} -> {:cont, {d, [c | acc]}}
    end)
    |> elem(1)
    |> Enum.reverse()
    |> Enum.join()
  end

  defp inner_parens(other), do: other

  defp arity(""), do: 0

  defp arity(args) do
    args
    |> strip_strings()
    |> String.graphemes()
    |> Enum.reduce({0, 1}, fn
      c, {d, n} when c in ["(", "[", "{", "<<"] -> {d + 1, n}
      c, {d, n} when c in [")", "]", "}", ">>"] -> {d - 1, n}
      ",", {0, n} -> {0, n + 1}
      _, acc -> acc
    end)
    |> elem(1)
  end

  # -- block ends --------------------------------------------------------------

  # First later `end` at exactly `indent`; falls back to the last line.
  defp find_block_end(all, from, indent) do
    all
    |> Enum.drop_while(fn {_t, no, _} -> no <= from end)
    |> Enum.find(fn {text, _no, skip} ->
      not skip and
        case Regex.named_captures(@end_re, text) do
          %{"indent" => i} -> String.length(i) == indent
          nil -> false
        end
    end)
    |> case do
      {_t, no, _} -> no
      nil -> all |> List.last() |> line_no()
    end
  end

  # A clause ends at the first later non-blank, non-skipped line whose
  # indentation is <= the def's indentation. If that line is the matching
  # `end`, the clause includes it; otherwise the clause ended on the previous
  # non-blank line.
  defp clause_end(all, head_end_no, indent) do
    after_head = Enum.drop_while(all, fn {_t, no, _} -> no <= head_end_no end)

    case Enum.find(after_head, fn {text, _no, _skip} ->
           String.trim(text) != "" and indentation(text) <= indent and
             not (indentation(text) == indent and Regex.match?(@block_continuation_re, text))
         end) do
      nil ->
        all |> List.last() |> line_no()

      {text, no, _} ->
        if Regex.match?(@end_re, text) and indentation(text) == indent do
          no
        else
          previous_non_blank(all, no) || head_end_no
        end
    end
  end

  defp previous_non_blank(all, before_no) do
    all
    |> Enum.take_while(fn {_t, no, _} -> no < before_no end)
    |> Enum.reverse()
    |> Enum.find(fn {text, _no, _} -> String.trim(text) != "" end)
    |> case do
      {_t, no, _} -> no
      nil -> nil
    end
  end

  defp indentation(text) do
    text |> String.replace(~r/^(\s*).*$/s, "\\1") |> String.length()
  end

  defp drop_until(lines, no), do: Enum.drop_while(lines, fn {_t, n, _} -> n <= no end)

  # `test "a \"quoted\" name"` -> a "quoted" name  (only the escapes that matter for a name)
  defp unescape(text),
    do:
      text
      |> String.replace("\\\"", "\"")
      |> String.replace("\\'", "'")
      |> String.replace("\\\\", "\\")

  defp line_no({_t, no, _skip}), do: no
end

defmodule CodeReviewer.Outline do
  @moduledoc """
  A structural outline of one Elixir source text: modules, the functions
  defined in them, and the module-level directives worth reviewing.

  Two parsers produce the same shape:

    * `CodeReviewer.Outline.AST` - exact, via `Code.string_to_quoted/2`.
      Needs syntactically complete source.
    * `CodeReviewer.Outline.Regex` - line oriented and indentation driven.
      Works on fragments and broken code; best effort on unusual layouts.

  `parse/2` tries the AST first and falls back to regex, recording which
  one produced the result in `parser`.
  """

  defmodule Clause do
    @moduledoc "One `def`/`defp` clause."
    @type t :: %__MODULE__{}
    defstruct [:head, :line_start, :line_end]
  end

  defmodule Function do
    @moduledoc "All clauses of one `name/arity` in a module."
    @type t :: %__MODULE__{}
    defstruct [
      :name,
      :arity,
      :kind,
      :visibility,
      :line_start,
      :line_end,
      clauses: [],
      annotations: []
    ]
  end

  defmodule Directive do
    @moduledoc "A module-level directive such as `alias Foo` or `attr :name, :string`."
    @type t :: %__MODULE__{}
    defstruct [:kind, :text, :line]
  end

  defmodule Module do
    @moduledoc "A `defmodule` with its functions and directives."
    @type t :: %__MODULE__{}
    defstruct [:name, :line_start, :line_end, functions: [], directives: []]
  end

  @type t :: %__MODULE__{parser: :ast | :regex, modules: [Module.t()]}
  defstruct parser: nil, modules: []

  @public ~w(def defmacro defguard defdelegate)a
  @private ~w(defp defmacrop defguardp)a
  @definers @public ++ @private

  @doc "The `def`-family keywords recognised as function definitions."
  def definers, do: @definers

  @doc """
  Maps a definer keyword to `:public` or `:private`.

  ExUnit `test` blocks are recorded as members of kind `:test` with the test
  description as their name and no arity; they count as public because they
  are the surface of a test module.
  """
  def visibility(kind) when kind in @public, do: :public
  def visibility(kind) when kind in @private, do: :private
  def visibility(:test), do: :public

  @doc """
  Outlines `source`, preferring the AST parser and falling back to regex.

  Options:

    * `:parser` - force `:ast` or `:regex`
  """
  @spec parse(String.t(), keyword()) :: t()
  def parse(source, opts \\ []) do
    case Keyword.get(opts, :parser, :auto) do
      :regex ->
        %__MODULE__{parser: :regex, modules: CodeReviewer.Outline.Regex.parse(source)}

      :ast ->
        {:ok, modules} = CodeReviewer.Outline.AST.parse(source)
        %__MODULE__{parser: :ast, modules: modules}

      :auto ->
        case CodeReviewer.Outline.AST.parse(source) do
          {:ok, modules} ->
            %__MODULE__{parser: :ast, modules: modules}

          {:error, _} ->
            %__MODULE__{parser: :regex, modules: CodeReviewer.Outline.Regex.parse(source)}
        end
    end
  end

  @doc "Shifts every line number in an outline through `mapper`, used for diff fragments."
  @spec remap_lines(t() | [Module.t()], (pos_integer() -> pos_integer())) :: t() | [Module.t()]
  def remap_lines(%__MODULE__{modules: modules} = outline, mapper),
    do: %{outline | modules: remap_lines(modules, mapper)}

  def remap_lines(modules, mapper) when is_list(modules) do
    Enum.map(modules, fn m ->
      %{
        m
        | line_start: mapper.(m.line_start),
          line_end: mapper.(m.line_end),
          directives: Enum.map(m.directives, &%{&1 | line: mapper.(&1.line)}),
          functions:
            Enum.map(m.functions, fn f ->
              %{
                f
                | line_start: mapper.(f.line_start),
                  line_end: mapper.(f.line_end),
                  clauses:
                    Enum.map(
                      f.clauses,
                      &%{&1 | line_start: mapper.(&1.line_start), line_end: mapper.(&1.line_end)}
                    ),
                  annotations: Enum.map(f.annotations, &%{&1 | line: mapper.(&1.line)})
              }
            end)
      }
    end)
  end

  @doc "Groups clause records into `Function`s by `{kind_group, name, arity}` preserving source order."
  @spec group_clauses([map()]) :: [Function.t()]
  def group_clauses(clauses) do
    clauses
    |> Enum.group_by(&{group(&1.kind), &1.name, &1.arity})
    |> Enum.map(fn {_key, [first | _] = cs} ->
      cs = Enum.sort_by(cs, & &1.line_start)
      # visibility follows the first clause; mixed def/defp is a compile error anyway
      %Function{
        name: first.name,
        arity: first.arity,
        kind: first.kind,
        visibility: visibility(first.kind),
        line_start: hd(cs).line_start,
        line_end: cs |> Enum.map(& &1.line_end) |> Enum.max(),
        clauses:
          Enum.map(cs, &%Clause{head: &1.head, line_start: &1.line_start, line_end: &1.line_end}),
        annotations:
          cs |> Enum.flat_map(&Map.get(&1, :annotations, [])) |> Enum.sort_by(& &1.line)
      }
    end)
    |> Enum.sort_by(& &1.line_start)
  end

  @doc "Groups definer kinds that share a namespace: def/defp, defmacro/defmacrop, defguard/defguardp, test."
  def group(kind) when kind in [:defmacro, :defmacrop], do: :macro
  def group(kind) when kind in [:defguard, :defguardp], do: :guard
  def group(:test), do: :test
  def group(_), do: :function
end

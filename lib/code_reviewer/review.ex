defmodule CodeReviewer.Review do
  @moduledoc """
  The structured result every renderer reads from. See `ideas-v09.md`.

  A review is a list of files; each file holds its diff hunks and the
  modules found in it; each module holds directives and functions. Every
  level carries a `verb` from the shared vocabulary:

    * `:created` - exists only after
    * `:deleted` - exists only before
    * `:updated` - exists on both sides and something inside differs
    * `:renamed` - exists on both sides under another name
    * `:unchanged` - exists on both sides, untouched by any hunk
  """

  @type verb :: :created | :deleted | :updated | :renamed | :unchanged

  defmodule Function do
    @moduledoc "One function (all clauses) as seen across the change."
    @type t :: %__MODULE__{}
    defstruct [
      :name,
      :arity,
      :visibility,
      :kind,
      :verb,
      details: [],
      range: %{before: nil, after: nil},
      clauses: %{before: 0, after: 0},
      heads: %{before: [], after: []},
      annotations: %{before: [], after: []},
      stats: %{added: 0, removed: 0}
    ]
  end

  defmodule Directive do
    @moduledoc "A module-level `alias`, `import`, `use`, `require`, `attr`, `slot` or `@moduledoc`/`@behaviour`."
    @type t :: %__MODULE__{}
    defstruct [:kind, :text, :verb, line: %{before: nil, after: nil}]
  end

  defmodule Module do
    @moduledoc "One module as seen across the change."
    @type t :: %__MODULE__{}
    defstruct [
      :name,
      :old_name,
      :verb,
      range: %{before: nil, after: nil},
      functions: [],
      directives: [],
      stats: %{added: 0, removed: 0}
    ]
  end

  defmodule File do
    @moduledoc "One file in the diff, with its hunks and the modules outlined in it."
    @type t :: %__MODULE__{}
    defstruct [
      :path,
      :old_path,
      :verb,
      :language,
      :parser,
      hunks: [],
      modules: [],
      stats: %{added: 0, removed: 0}
    ]
  end

  @type t :: %__MODULE__{}
  defstruct source: %{}, files: []
end

defmodule CodeReviewerWeb.HomeLive.Index do
  @moduledoc """
  The flat function index (`ideas-v04.md`) as a keyboard-driven table.

  Functions stay the unit of data; the table rolls them up under a row per
  module. Both kinds of row live in one stream, in display order, with
  function rows pointing at their module through `parent_id`.

  The review is computed off the LiveView process with `start_async/3`.
  Cursor movement and expand/collapse are client side in the `VimGrid` hook.
  `Enter` on a function row asks the server for that function's diff
  (`"toggle_diff"`); the diff is built on demand from the two file versions
  in git and streamed in as a row right under the function. `Enter` on a
  module row folds it, except on its `+/-` cell, which opens the same kind
  of diff for the whole module.
  """

  use CodeReviewerWeb, :live_view

  alias CodeReviewer.{FunctionDiff, FunctionIndex, Git}

  # Fixed table layout: every column but Function has a width, Function takes
  # the rest. Long text truncates instead of wrapping so rows stay one line
  # high, which keeps j/k movement predictable.
  @columns [
    %{key: "verb", label: "Verb", width: "w-16", align: nil},
    %{key: "module", label: "Module / Function", width: nil, align: nil},
    %{key: "stats", label: "+ / −", width: "w-28", align: "text-center"},
    %{key: "notes", label: "Notes", width: "w-44", align: nil},
    %{key: "lines", label: "Lines", width: "w-28", align: nil},
    %{key: "location", label: "File:Line", width: "w-[18rem]", align: nil}
  ]

  @impl true
  def mount(_params, _session, socket) do
    socket =
      socket
      |> assign(:page_title, "Functions")
      |> assign(:columns, @columns)
      |> assign(:status, :loading)
      |> assign(:error, nil)
      |> assign(:summary, nil)
      |> assign(:title, nil)
      |> assign(:row_count, 0)
      |> assign(:module_count, 0)
      |> assign(:selected, nil)
      |> assign(:review, nil)
      |> assign(:rows_by_id, %{})
      |> assign(:display_order, [])
      |> assign(:open_diffs, MapSet.new())
      |> stream(:rows, [])

    {:ok, socket}
  end

  @impl true
  def handle_params(params, _uri, socket) do
    source = source(params)

    socket =
      socket
      |> assign(:source, source)
      |> assign(:form, to_form(%{"repo" => source.repo, "rev" => source.rev}))
      |> assign(:status, :loading)
      |> assign(:error, nil)
      |> assign(:selected, nil)
      |> start_async(:load, fn -> load(source) end)

    {:noreply, socket}
  end

  @impl true
  def handle_async(:load, {:ok, {:ok, result}}, socket) do
    rows = flatten(result.modules)

    socket =
      socket
      |> assign(:status, :ready)
      |> assign(:summary, result.summary)
      |> assign(:title, result.title)
      |> assign(:module_count, length(result.modules))
      |> assign(:row_count, result.summary.total)
      |> assign(:review, result.review)
      |> assign(:rows_by_id, Map.new(rows, &{&1.id, &1}))
      |> assign(:display_order, Enum.map(rows, & &1.id))
      |> assign(:open_diffs, MapSet.new())
      |> stream(:rows, rows, reset: true)

    {:noreply, socket}
  end

  def handle_async(:load, {:ok, {:error, reason}}, socket) do
    {:noreply,
     socket
     |> assign(:status, :failed)
     |> assign(:error, to_string(reason))
     |> stream(:rows, [], reset: true)}
  end

  def handle_async(:load, {:exit, reason}, socket) do
    {:noreply,
     socket
     |> assign(:status, :failed)
     |> assign(:error, Exception.format_exit(reason))
     |> stream(:rows, [], reset: true)}
  end

  @impl true
  def handle_event("source", %{"repo" => repo, "rev" => rev}, socket) do
    {:noreply, push_patch(socket, to: ~p"/?#{%{repo: String.trim(repo), rev: String.trim(rev)}}")}
  end

  # `Enter` on a function row, or on a module row's `+/-`: show or hide the
  # diff row for that function or module.
  def handle_event("toggle_diff", %{"id" => id}, socket) do
    diff_id = "d-" <> id

    cond do
      MapSet.member?(socket.assigns.open_diffs, id) ->
        socket =
          socket
          |> assign(:open_diffs, MapSet.delete(socket.assigns.open_diffs, id))
          |> assign(:display_order, List.delete(socket.assigns.display_order, diff_id))
          |> stream_delete(:rows, %{id: diff_id})

        {:noreply, socket}

      row = socket.assigns.rows_by_id[id] ->
        diff_row = diff_row(socket.assigns, row, diff_id)
        position = Enum.find_index(socket.assigns.display_order, &(&1 == id)) + 1

        socket =
          socket
          |> assign(:open_diffs, MapSet.put(socket.assigns.open_diffs, id))
          |> assign(
            :display_order,
            List.insert_at(socket.assigns.display_order, position, diff_id)
          )
          # the stream container's first child is the empty-state row
          |> stream_insert(:rows, diff_row, at: position + 1)

        {:noreply, socket}

      true ->
        {:noreply, socket}
    end
  end

  # `Enter` on any other cell. Behaviour to be decided; for now the server
  # records what was chosen so the round trip is visible.
  def handle_event("activate", %{"id" => id, "column" => column} = params, socket) do
    selected = %{id: id, column: column, module: params["module"], function: params["function"]}
    {:noreply, assign(socket, :selected, selected)}
  end

  # -- data --------------------------------------------------------------------

  defp source(params) do
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

  # The repository this app lives in, so the page shows something on first load.
  defp default_repo do
    Git.toplevel(File.cwd!()) || File.cwd!()
  end

  defp load(%{repo: repo, rev: rev}) do
    cond do
      not File.dir?(repo) ->
        {:error, "#{repo} is not a directory"}

      Git.toplevel(repo) == nil ->
        {:error, "#{repo} is not inside a git repository"}

      true ->
        case CodeReviewer.review_git(repo, from: "#{rev}~1", to: rev) do
          {:ok, review} ->
            modules = FunctionIndex.by_module(review)
            rows = Enum.flat_map(modules, & &1.functions)

            {:ok,
             %{
               review: review,
               modules: modules,
               summary: FunctionIndex.summary(rows),
               title: review.source[:title]
             }}

          {:error, reason} ->
            {:error, reason}
        end
    end
  end

  # One stream in display order: a module row, then that module's functions.
  defp flatten(modules) do
    Enum.flat_map(modules, fn m ->
      module_row = %{
        id: "m-" <> m.id,
        kind: :module,
        parent_id: nil,
        module: m.module,
        verb: m.verb,
        file: m.file,
        line: m.line,
        ranges: m.ranges,
        stats: m.stats,
        function_count: length(m.functions)
      }

      [
        module_row
        | Enum.map(
            m.functions,
            # the row kind drives rendering; the definer (def/defp/test) is kept aside
            &Map.merge(&1, %{kind: :function, definer: &1.kind, parent_id: module_row.id})
          )
      ]
    end)
  end

  # Whole-function (or whole-module) diff, built from the two file versions
  # fetched from git. Module diffs sit under the module row and stay visible
  # when it is folded; function diffs fold with their module.
  defp diff_row(%{review: review, source: source}, row, diff_id) do
    file = Enum.find(review.files, &(&1.path == row.file))
    %{from: from, to: to} = revisions(source)
    before_src = file && fetch(source.repo, from, file.old_path || file.path)
    after_src = file && fetch(source.repo, to, file.path)

    lines =
      if file,
        do:
          FunctionDiff.for_ranges(
            file,
            row.ranges.before,
            row.ranges.after,
            before_src,
            after_src
          ),
        else: []

    %{
      id: diff_id,
      kind: :diff,
      parent_id: row.id,
      fold_id: row.parent_id,
      module: row.module,
      function: row[:function],
      lines: lines
    }
  end

  defp revisions(%{rev: rev}), do: %{from: "#{rev}~1", to: rev}

  defp fetch(repo, rev, path) do
    case Git.show(repo, rev, path) do
      {:ok, text} -> text
      {:error, _} -> nil
    end
  end

  # -- view helpers -----------------------------------------------------------------

  @doc false
  def line_class(:add), do: "bg-emerald-500/10 text-emerald-900 dark:text-emerald-100"
  def line_class(:del), do: "bg-rose-500/10 text-rose-900 dark:text-rose-100"
  def line_class(:context), do: "text-base-content/80"

  # Plain ink for a real count, light gray when there is nothing to report.
  @doc false
  def count_class(0, _kind), do: "text-base-content/35"
  def count_class(_n, :add), do: "text-emerald-500/60 dark:text-emerald-400/60"
  def count_class(_n, :del), do: "text-rose-500/60 dark:text-rose-400/60"

  @doc false
  def verb_letter(verb), do: verb |> Atom.to_string() |> String.first() |> String.upcase()

  @doc false
  def marker(:add), do: "+"
  def marker(:del), do: "-"
  def marker(:context), do: ""

  @doc false
  def verb_class(:created), do: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
  def verb_class(:deleted), do: "bg-rose-500/10 text-rose-700 dark:text-rose-300"
  def verb_class(:updated), do: "bg-blue-500/10 text-blue-700 dark:text-blue-300"
  def verb_class(:renamed), do: "bg-violet-500/10 text-violet-700 dark:text-violet-300"
  def verb_class(_), do: "bg-base-300 text-base-content/70"

  # What changed about an updated function besides its body, as short notes.
  @doc false
  def notes(%{verb: :updated, details: details, clauses: clauses}) do
    details
    |> Enum.flat_map(fn
      :clauses -> ["#{clauses.before} → #{clauses.after} clauses"]
      :visibility -> ["visibility changed"]
      :head -> ["head changed"]
      :spec -> ["spec changed"]
      :doc -> ["doc changed"]
      :impl -> ["impl changed"]
      _ -> []
    end)
    |> Enum.join(", ")
  end

  def notes(%{clauses: %{after: a}}) when a > 1, do: "#{a} clauses"
  def notes(%{clauses: %{before: b, after: 0}}) when b > 1, do: "#{b} clauses"
  def notes(_row), do: ""

  # `def rows/2`, `defp module_id/1`, `test "name"`
  @doc false
  def function_label(%{definer: :test, function: function}), do: function
  def function_label(%{definer: definer, function: function}), do: "#{definer} #{function}"

  @doc false
  def lines(%{range: nil}), do: ""
  def lines(%{range: range, side: :before}), do: "was #{range.first}-#{range.last}"
  def lines(%{range: range}), do: "#{range.first}-#{range.last}"

  @doc false
  def location(%{file: file, line: nil}), do: file
  def location(%{file: file, line: line}), do: "#{file}:#{line}"

  @doc false
  def selected_label(%{module: module, function: nil}), do: module || "—"
  def selected_label(%{module: module, function: function}), do: "#{module}.#{function}"

  @doc false
  def short_repo(path) do
    home = System.user_home!()

    if String.starts_with?(path, home),
      do: "~" <> String.replace_prefix(path, home, ""),
      else: path
  end
end

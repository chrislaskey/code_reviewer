defmodule CodeReviewerWeb.HomeLive.Index do
  @moduledoc """
  The flat function index (`ideas-v04.md`) as a keyboard-driven table.

  Functions stay the unit of data; the table rolls them up under a row per
  module. Both kinds of row live in one stream, in display order, with
  function rows pointing at their module through `parent_id`.

  The review is computed off the LiveView process with `start_async/3`.
  Cursor movement and expand/collapse are client side in the `VimGrid` hook;
  only `Enter` on a non-module cell reaches the server, as `"activate"`.
  """

  use CodeReviewerWeb, :live_view

  alias CodeReviewer.{FunctionIndex, Git}

  # Fixed table layout: every column but Function has a width, Function takes
  # the rest. Long text truncates instead of wrapping so rows stay one line
  # high, which keeps j/k movement predictable.
  @columns [
    %{key: "module", label: "Module", width: "w-[26rem]"},
    %{key: "verb", label: "Verb", width: "w-24"},
    %{key: "visibility", label: "Vis", width: "w-20"},
    %{key: "function", label: "Function", width: nil},
    %{key: "clauses", label: "Clauses", width: "w-20"},
    %{key: "lines", label: "Lines", width: "w-28"},
    %{key: "stats", label: "+/-", width: "w-24"},
    %{key: "location", label: "File:Line", width: "w-[18rem]"}
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
    socket =
      socket
      |> assign(:status, :ready)
      |> assign(:summary, result.summary)
      |> assign(:title, result.title)
      |> assign(:module_count, length(result.modules))
      |> assign(:row_count, result.summary.total)
      |> stream(:rows, flatten(result.modules), reset: true)

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

  # `Enter` on a cell. Behaviour to be decided; for now the server records
  # what was chosen so the round trip is visible.
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
        stats: m.stats,
        function_count: length(m.functions)
      }

      [
        module_row
        | Enum.map(m.functions, &Map.merge(&1, %{kind: :function, parent_id: module_row.id}))
      ]
    end)
  end

  # -- view helpers -----------------------------------------------------------------

  @doc false
  def verb_class(:created), do: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
  def verb_class(:deleted), do: "bg-rose-500/10 text-rose-700 dark:text-rose-300"
  def verb_class(:updated), do: "bg-blue-500/10 text-blue-700 dark:text-blue-300"
  def verb_class(:renamed), do: "bg-violet-500/10 text-violet-700 dark:text-violet-300"
  def verb_class(_), do: "bg-base-300 text-base-content/70"

  @doc false
  def clauses(%{clauses: %{before: b, after: a}, verb: :created}) when b == 0, do: "#{a}/-"
  def clauses(%{clauses: %{before: b, after: a}, verb: :deleted}) when a == 0, do: "-/#{b}"
  def clauses(%{clauses: %{before: b, after: a}}), do: "#{a}/#{b}"

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

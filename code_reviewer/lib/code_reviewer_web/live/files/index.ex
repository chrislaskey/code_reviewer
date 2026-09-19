defmodule CodeReviewerWeb.FilesLive.Index do
  @moduledoc """
  The file-by-file view, modelled on a GitHub pull request's "Files changed"
  tab: a file tree down the left, and one card per file on the right with its
  hunks laid out as a unified diff.

  Files are streamed so that marking one viewed or folding it re-renders only
  that card. Per-file state (`collapsed`, `viewed`) lives on the server in
  `files_by_id`; the tree and the header read counts from it.
  """

  use CodeReviewerWeb, :live_view

  import CodeReviewerWeb.ReviewComponents

  alias CodeReviewerWeb.ReviewSource

  @impl true
  def mount(_params, _session, socket) do
    socket =
      socket
      |> assign(:page_title, "Files")
      |> assign(:status, :loading)
      |> assign(:error, nil)
      |> assign(:title, nil)
      |> assign(:summary, nil)
      |> assign(:tree, [])
      |> assign(:files_by_id, %{})
      |> assign(:order, [])
      |> assign(:viewed_count, 0)
      |> assign(:selected, nil)
      |> stream_configure(:files, dom_id: & &1.id)
      |> stream(:files, [])

    {:ok, socket}
  end

  @impl true
  def handle_params(params, _uri, socket) do
    source = ReviewSource.from_params(params)

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
      |> assign(:title, result.title)
      |> assign(:summary, result.summary)
      |> assign(:tree, result.tree)
      |> assign(:files_by_id, Map.new(result.files, &{&1.id, &1}))
      |> assign(:order, Enum.map(result.files, & &1.id))
      |> assign(:viewed_count, 0)
      |> stream(:files, result.files, reset: true)

    {:noreply, socket}
  end

  def handle_async(:load, {:ok, {:error, reason}}, socket) do
    {:noreply, failed(socket, to_string(reason))}
  end

  def handle_async(:load, {:exit, reason}, socket) do
    {:noreply, failed(socket, Exception.format_exit(reason))}
  end

  defp failed(socket, error) do
    socket
    |> assign(:status, :failed)
    |> assign(:error, error)
    |> assign(:tree, [])
    |> stream(:files, [], reset: true)
  end

  @impl true
  def handle_event("source", %{"repo" => repo, "rev" => rev}, socket) do
    to = ~p"/files?#{%{repo: String.trim(repo), rev: String.trim(rev)}}"
    {:noreply, push_patch(socket, to: to)}
  end

  # The chevron in a file's header folds the diff away and back.
  def handle_event("toggle_collapsed", %{"id" => id}, socket) do
    {:noreply, update_file(socket, id, fn f -> %{f | collapsed: not f.collapsed} end)}
  end

  # The "Viewed" checkbox. As on GitHub, a viewed file folds; unticking it
  # unfolds it again.
  def handle_event("toggle_viewed", %{"id" => id}, socket) do
    {:noreply,
     update_file(socket, id, fn f -> %{f | viewed: not f.viewed, collapsed: not f.viewed} end)}
  end

  # `Enter` on a diff line. Behaviour to be decided; for now the server
  # records what was chosen so the round trip is visible.
  def handle_event("activate", %{"path" => path, "line" => %{"kind" => kind} = line}, socket)
      when kind in ~w(add del context) do
    selected = %{
      path: path,
      line: %{kind: String.to_existing_atom(kind), old: line["old"], new: line["new"]}
    }

    {:noreply, assign(socket, :selected, selected)}
  end

  def handle_event("activate", _params, socket), do: {:noreply, socket}

  def handle_event("expand_all", _params, socket) do
    {:noreply, update_files(socket, fn f -> %{f | collapsed: false} end)}
  end

  def handle_event("collapse_all", _params, socket) do
    {:noreply, update_files(socket, fn f -> %{f | collapsed: true} end)}
  end

  defp update_file(socket, id, fun) do
    case socket.assigns.files_by_id[id] do
      nil ->
        socket

      file ->
        file = fun.(file)

        socket
        |> assign(:files_by_id, Map.put(socket.assigns.files_by_id, id, file))
        |> stream_insert(:files, file)
        |> recount()
    end
  end

  defp update_files(socket, fun) do
    files = Enum.map(socket.assigns.order, &fun.(socket.assigns.files_by_id[&1]))

    socket
    |> assign(:files_by_id, Map.new(files, &{&1.id, &1}))
    |> stream(:files, files, reset: true)
    |> recount()
  end

  defp recount(socket) do
    viewed = socket.assigns.files_by_id |> Map.values() |> Enum.count(& &1.viewed)
    assign(socket, :viewed_count, viewed)
  end

  # -- data --------------------------------------------------------------------

  defp load(source) do
    with {:ok, review} <- ReviewSource.review(source) do
      files = Enum.map(review.files, &file_entry/1)

      {:ok,
       %{
         title: review.source[:title],
         summary: summary(files),
         tree: tree(files),
         files: files
       }}
    end
  end

  # One stream item per file. Everything the card needs is here so a
  # `stream_insert` can re-render it alone.
  defp file_entry(file) do
    %{
      id: "f-" <> Base.url_encode64(file.path, padding: false),
      path: file.path,
      old_path: file.old_path,
      dir: Path.dirname(file.path),
      name: Path.basename(file.path),
      verb: file.verb,
      language: file.language,
      stats: file.stats,
      hunks: file.hunks,
      collapsed: false,
      viewed: false
    }
  end

  defp summary(files) do
    %{
      total: length(files),
      by_verb:
        Map.merge(
          %{created: 0, updated: 0, deleted: 0, renamed: 0},
          Enum.frequencies_by(files, & &1.verb)
        ),
      added: files |> Enum.map(& &1.stats.added) |> Enum.sum(),
      removed: files |> Enum.map(& &1.stats.removed) |> Enum.sum()
    }
  end

  # -- tree --------------------------------------------------------------------

  # The sidebar tree: nested directories with files as leaves, directories
  # first, everything sorted by name. A directory whose only child is another
  # directory is joined with it (`lib/code_reviewer_web/live`), as GitHub does,
  # so deep paths do not turn into a staircase of single entries.
  @doc false
  def tree(files) do
    files
    |> Enum.map(&{Path.split(&1.path), &1})
    |> build_tree()
    |> compact()
  end

  defp build_tree(entries) do
    {leaves, branches} = Enum.split_with(entries, fn {parts, _} -> length(parts) == 1 end)

    dirs =
      branches
      |> Enum.group_by(fn {[head | _], _} -> head end, fn {[_ | rest], file} -> {rest, file} end)
      |> Enum.map(fn {name, children} ->
        %{kind: :dir, name: name, children: build_tree(children)}
      end)
      |> Enum.sort_by(& &1.name)

    files =
      leaves
      |> Enum.map(fn {[name], file} ->
        %{kind: :file, name: name, id: file.id, verb: file.verb, stats: file.stats}
      end)
      |> Enum.sort_by(& &1.name)

    dirs ++ files
  end

  defp compact(nodes) do
    Enum.map(nodes, fn
      %{kind: :dir, children: [%{kind: :dir} = only]} = dir ->
        compact([%{only | name: dir.name <> "/" <> only.name}]) |> hd()

      %{kind: :dir} = dir ->
        %{dir | children: compact(dir.children)}

      file ->
        file
    end)
  end

  # -- components --------------------------------------------------------------

  # One node of the sidebar tree. Directories are native `<details>` so they
  # fold without any JavaScript; files link to their card.
  attr :node, :map, required: true
  attr :depth, :integer, default: 0

  defp tree_node(%{node: %{kind: :dir}} = assigns) do
    ~H"""
    <li>
      <details open>
        <summary
          class="flex cursor-pointer select-none items-center gap-1.5 py-1 pr-2 text-xs text-base-content/80 transition-colors hover:bg-base-200/60"
          style={indent(@depth)}
          title={@node.name}
        >
          <.icon
            name="hero-chevron-down-mini"
            class="tree-chevron size-3.5 shrink-0 text-base-content/50 transition-transform"
          />
          <.icon name="hero-folder-mini" class="size-3.5 shrink-0 text-base-content/40" />
          <span class="truncate font-mono">{@node.name}</span>
        </summary>
        <ul>
          <.tree_node :for={child <- @node.children} node={child} depth={@depth + 1} />
        </ul>
      </details>
    </li>
    """
  end

  defp tree_node(assigns) do
    ~H"""
    <li>
      <a
        href={"#" <> @node.id}
        data-file={@node.id}
        class="flex items-center gap-2 py-1 pr-2 text-xs text-base-content/80 transition-colors hover:bg-base-200/60 hover:text-base-content"
        style={indent(@depth + 1)}
        title={@node.name}
      >
        <span class={["size-2 shrink-0 rounded-full", dot_class(@node.verb)]}></span>
        <span class="min-w-0 truncate font-mono">{@node.name}</span>
        <span class="ml-auto shrink-0 font-mono text-[11px] tabular-nums text-base-content/40">
          <span :if={@node.stats.added > 0} class="text-emerald-600/70 dark:text-emerald-400/70">+{@node.stats.added}</span>
          <span :if={@node.stats.removed > 0} class="text-rose-600/70 dark:text-rose-400/70">−{@node.stats.removed}</span>
        </span>
      </a>
    </li>
    """
  end

  defp indent(depth), do: "padding-left: #{0.5 + depth * 0.875}rem"

  # -- view helpers ------------------------------------------------------------

  @doc false
  def line_class(:add), do: "bg-emerald-500/10 text-emerald-900 dark:text-emerald-100"
  def line_class(:del), do: "bg-rose-500/10 text-rose-900 dark:text-rose-100"
  def line_class(:context), do: "text-base-content/80"

  @doc false
  def gutter_class(:add), do: "bg-emerald-500/15 text-emerald-700/70 dark:text-emerald-300/70"
  def gutter_class(:del), do: "bg-rose-500/15 text-rose-700/70 dark:text-rose-300/70"
  def gutter_class(:context), do: "text-base-content/40"

  @doc false
  def marker(:add), do: "+"
  def marker(:del), do: "-"
  def marker(:context), do: ""

  # `+42` for an added line, `−17` for a removed one, `42` for context.
  @doc false
  def selected_line(%{kind: :add, new: new}), do: "+#{new}"
  def selected_line(%{kind: :del, old: old}), do: "−#{old}"
  def selected_line(%{new: new, old: old}), do: to_string(new || old || "—")

  @doc false
  def hunk_label(hunk) do
    "@@ -#{hunk.old_start},#{hunk.old_count} +#{hunk.new_start},#{hunk.new_count} @@"
  end

  # A small coloured dot per verb for the tree, where a badge is too loud.
  @doc false
  def dot_class(:created), do: "bg-emerald-500"
  def dot_class(:deleted), do: "bg-rose-500"
  def dot_class(:updated), do: "bg-blue-500"
  def dot_class(:renamed), do: "bg-violet-500"
  def dot_class(_), do: "bg-base-content/30"

  @doc false
  def verb_word(:created), do: "added"
  def verb_word(:deleted), do: "removed"
  def verb_word(:renamed), do: "renamed"
  def verb_word(:updated), do: "modified"
  def verb_word(other), do: Atom.to_string(other)
end

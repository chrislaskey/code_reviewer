defmodule CodeReviewerWeb.ReviewComponents do
  @moduledoc """
  Function components shared by the review views: the repository picker that
  sits above every view, and the verb badge used wherever a change is labelled.
  """

  use Phoenix.Component

  alias CodeReviewerWeb.ReviewSource

  @doc """
  The repository and revision picker. Submits a `"source"` event with `repo`
  and `rev`; the view turns that into a `push_patch` so the URL stays the
  source of truth.
  """
  attr :form, Phoenix.HTML.Form, required: true
  attr :source, :map, required: true, doc: "the current %{repo: path, rev: rev}"

  def source_picker(assigns) do
    ~H"""
    <.form
      for={@form}
      id="source-form"
      phx-submit="source"
      class="flex flex-wrap items-end gap-3 rounded-lg border border-base-300 bg-base-200/40 px-4 py-3"
    >
      <label class="flex flex-col gap-1 text-xs text-base-content/60">
        Repository
        <input
          type="text"
          name="repo"
          id="source-repo"
          value={@form[:repo].value}
          spellcheck="false"
          class="w-[28rem] max-w-full rounded-md border border-base-300 bg-base-100 px-2.5 py-1.5 font-mono text-sm text-base-content focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
        />
      </label>
      <label class="flex flex-col gap-1 text-xs text-base-content/60">
        Revision
        <input
          type="text"
          name="rev"
          id="source-rev"
          value={@form[:rev].value}
          spellcheck="false"
          class="w-40 rounded-md border border-base-300 bg-base-100 px-2.5 py-1.5 font-mono text-sm text-base-content focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
        />
      </label>
      <button
        type="submit"
        id="source-load"
        class="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-content transition hover:brightness-110 active:scale-[0.98]"
      >
        Load
      </button>
      <p class="ml-auto self-center text-xs text-base-content/50">
        Compares <span class="font-mono">{@source.rev}~1</span>
        to <span class="font-mono">{@source.rev}</span>
      </p>
    </.form>
    """
  end

  @doc "A one-letter badge for a verb: `C`, `U`, `D`, `R`, tinted to match."
  attr :verb, :atom, required: true

  def verb_badge(assigns) do
    ~H"""
    <span
      class={[
        "inline-block w-5 rounded py-0.5 text-center text-xs font-semibold",
        verb_class(@verb)
      ]}
      title={@verb |> Atom.to_string() |> String.capitalize()}
    >
      {verb_letter(@verb)}
    </span>
    """
  end

  @doc false
  def verb_letter(verb), do: verb |> Atom.to_string() |> String.first() |> String.upcase()

  @doc false
  def verb_class(:created), do: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
  def verb_class(:deleted), do: "bg-rose-500/10 text-rose-700 dark:text-rose-300"
  def verb_class(:updated), do: "bg-blue-500/10 text-blue-700 dark:text-blue-300"
  def verb_class(:renamed), do: "bg-violet-500/10 text-violet-700 dark:text-violet-300"
  def verb_class(_), do: "bg-base-300 text-base-content/70"

  @doc false
  defdelegate short_repo(path), to: ReviewSource
end

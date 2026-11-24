defmodule App.Data.Tagging do
  @moduledoc """
  Tags and tagging posts, over the `App.Data.Tags` and
  `App.Data.PostsTags` contexts. Both change through events, applied by
  `App.Data.EventFlow` with the specs here; the web writes them with
  `App.Data.Sync.save/4` as a device would. See
  `docs/design/related-resources.md`.

  A tag is its owner's: only they rename or delete it, and it goes only on
  their own posts, so its name never follows someone else's post into a
  community. Deleting a tag copies its `deleted_at` onto its posts_tags
  rows and leaves the posts, see `App.Data.Deletes`.

  A tag's events are recorded in its own stream, and a posts_tags row's
  in its post's.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.EventFlow
  alias App.Data.Posting
  alias App.Data.PostsTags
  alias App.Data.PostTag
  alias App.Data.Sync
  alias App.Data.Tag
  alias App.Data.Tags
  alias App.Web.Scope

  @doc """
  Applies an event from a device or the web to a tag or a posts_tags
  row, checked for shape by `App.Data.Sync`. See `App.Data.EventFlow`
  for what it returns.
  """
  def apply_event(%Scope{} = scope, %{table: table} = event), do: EventFlow.apply(scope, event, spec(table))

  defp spec("app_tags") do
    %{
      schema: Tag,
      context: Tags,
      fields: %{create: ["name"], update: ["name", "deleted_at"]},
      unknown: "unknown_tag",
      normalize: &trim_name/1,
      check_create: fn %Scope{user: user}, _changes -> {:ok, owner_id: user.id} end,
      check_update: fn
        %Scope{user: %{id: owner_id}}, %Tag{owner_id: owner_id}, _changes -> :ok
        _scope, _tag, _changes -> {:error, "not_owner"}
      end
    }
  end

  defp spec("app_posts_tags") do
    %{
      schema: PostTag,
      context: PostsTags,
      fields: %{
        create: ["post_id", "tag_id", "position", "note"],
        update: ["position", "note", "deleted_at"]
      },
      unknown: "unknown_post_tag",
      check_create: fn scope, changes ->
        with :ok <- Posting.own_post(scope, changes["post_id"]),
             :ok <- own_tag(scope, changes["tag_id"]),
             nil <- pair(changes["post_id"], changes["tag_id"]) do
          {:ok, []}
        else
          %PostTag{} = row -> {:error, "exists", row}
          error -> error
        end
      end,
      check_update: fn scope, row, _changes ->
        with {:error, reason, _row} <- Posting.own_post(scope, row.post_id), do: {:error, reason}
      end
    }
  end

  defp trim_name(%{"name" => name} = changes) when is_binary(name),
    do: %{changes | "name" => String.trim(name)}

  defp trim_name(changes), do: changes

  defp own_tag(%Scope{user: user}, tag_id) do
    case tag_id && Ecto.UUID.cast(tag_id) do
      {:ok, id} ->
        case Tags.get(id, include_deleted: true) do
          nil -> {:error, "unknown_tag", nil}
          %Tag{owner_id: owner_id} when owner_id == user.id -> :ok
          %Tag{} -> {:error, "not_own_tag", nil}
        end

      _not_an_id ->
        {:error, "unknown_tag", nil}
    end
  end

  # The pair's row, removed or not: a pair has one row for good
  defp pair(post_id, tag_id), do: PostsTags.get_by([post_id: post_id, tag_id: tag_id], include_deleted: true)

  @doc """
  Tags the post with the scope's user's tag of that name, making the tag
  first when there is none, and placing it after the post's other tags.
  Tagging a post again revives its earlier row. As the web does it: the
  events a device would send, applied at once, all or nothing. Returns
  `{:ok, post_tag}` or the error of `App.Data.Sync.save/4`.
  """
  def tag_post(%Scope{} = scope, post, name) do
    App.Repo.transact(fn ->
      with {:ok, tag} <- find_or_make(scope, name) do
        tag_with(scope, post, tag)
      end
    end)
  end

  defp tag_with(scope, post, tag) do
    position = length(tags_of(post.id))

    result =
      case pair(post.id, tag.id) do
        nil ->
          Sync.save(scope, "app_posts_tags", nil, %{
            "post_id" => post.id,
            "tag_id" => tag.id,
            "position" => position
          })

        %PostTag{deleted_at: nil} = row ->
          {:ok, row}

        row ->
          Sync.save(scope, "app_posts_tags", row, %{"deleted_at" => nil, "position" => position})
      end

    case result do
      {:conflict, _conflicts, row} -> {:ok, row}
      result -> result
    end
  end

  defp find_or_make(scope, name) do
    case tag_named(scope, name) do
      nil -> Sync.save(scope, "app_tags", nil, %{"name" => name})
      tag -> {:ok, tag}
    end
  end

  @doc """
  Removes the tag from the post: soft-deletes the posts_tags row.
  """
  def untag(%Scope{} = scope, %PostTag{} = row) do
    Sync.save(scope, "app_posts_tags", row, %{"deleted_at" => DateTime.utc_now()})
  end

  @doc """
  Renames the tag.
  """
  def rename_tag(%Scope{} = scope, %Tag{} = tag, name),
    do: Sync.save(scope, "app_tags", tag, %{"name" => name})

  @doc """
  Deletes the tag, which takes it off its posts.
  """
  def delete_tag(%Scope{} = scope, %Tag{} = tag) do
    Sync.save(scope, "app_tags", tag, %{"deleted_at" => DateTime.utc_now()})
  end

  @doc """
  The scope's user's live tags by name, each with how many live posts
  carry it, as `{tag, count}`.
  """
  def tags_with_counts(%Scope{user: user}) do
    counts =
      App.Repo.all(
        from(pt in PostTag,
          join: post in assoc(pt, :post),
          where: is_nil(pt.deleted_at) and is_nil(post.deleted_at),
          group_by: pt.tag_id,
          select: {pt.tag_id, count(pt.id)}
        )
      )
      |> Map.new()

    for tag <- Tags.list(filters: [owner_id: user.id]), do: {tag, Map.get(counts, tag.id, 0)}
  end

  @doc """
  The live tags of the post in the order it shows them, by position then
  name, as posts_tags rows with their tags.
  """
  def tags_of(post_id), do: PostsTags.list(filters: [post_id: post_id], preloads: [:tag])

  @doc """
  The scope's user's live tag with the name, ignoring case, or nil.
  """
  def tag_named(%Scope{user: user}, name) do
    name = String.trim(name)

    App.Repo.one(
      from(t in Tag,
        where:
          t.owner_id == ^user.id and is_nil(t.deleted_at) and
            fragment("lower(?)", t.name) == ^String.downcase(name)
      )
    )
  end
end

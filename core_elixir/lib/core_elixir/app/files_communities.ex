defmodule CoreElixir.App.FilesCommunities do
  @moduledoc """
  A file in a community: what a pushed `app_files_communities` event and
  Axum's routes do. Each is a command to the file's aggregate, which
  revives a link rather than making a second one, and deletes the file
  when its last link goes.
  """
  alias CoreElixir.App.Files.Commands.{AddFileToCommunity, RemoveFileFromCommunity}
  alias CoreElixir.CommandedApplication
  alias CoreElixir.Data.{AppEvents, AppFiles, AppFilesCommunities, AppUsersCommunities}
  alias CoreElixir.Helpers.{Ids, Time}
  alias CoreElixir.Sync.{Answers, Merge}

  @create_fields ~w(file_id community_id)
  @update_fields ~w(deleted_at)

  @doc """
  Why the links a file's create carries are refused, if they are: there
  must be one at least, each in a community the actor is a live member
  of, under an id no row has. Nil when they are fine.
  """
  def refusal_of_new(id, actor_id, links) do
    community_ids = Enum.map(links, & &1["community_id"])

    cond do
      links == [] ->
        Answers.rejected(id, "no_community")

      length(Enum.uniq(community_ids)) != length(community_ids) ->
        Answers.rejected(id, "invalid: a community is named twice")

      Enum.any?(community_ids, &is_nil(AppUsersCommunities.get_live(actor_id, &1))) ->
        Answers.rejected(id, "not_a_member")

      Enum.any?(links, &AppFilesCommunities.get(&1["id"])) ->
        Answers.rejected(id, "exists")

      true ->
        nil
    end
  end

  @doc "Applies a pushed event of the actor's to a file's link and answers it."
  def apply_event(actor_id, %{"id" => id} = event) do
    case AppEvents.get(id) do
      nil -> decide(actor_id, event)
      recorded -> Answers.from_record(recorded, server_row(AppFilesCommunities.get(recorded.row_id)))
    end
  end

  defp decide(actor_id, %{"type" => "create"} = event) do
    %{"id" => id, "row_id" => link_id, "changes" => changes} = event
    file_id = changes["file_id"]
    community_id = changes["community_id"]

    cond do
      reason = Merge.not_editable(changes, @create_fields) ->
        Answers.rejected(id, reason)

      not (uuid?(file_id) and AppFiles.get_visible(actor_id, file_id)) ->
        Answers.rejected(id, "unknown_file")

      not (uuid?(community_id) and AppUsersCommunities.get_live(actor_id, community_id)) ->
        Answers.rejected(id, "not_a_member")

      true ->
        case AppFilesCommunities.get_by_file_and_community(file_id, community_id) do
          %AppFilesCommunities{deleted_at: nil} = existing ->
            Answers.applied(id, existing.version, %{}, server_row(existing))

          nil ->
            if AppFilesCommunities.get(link_id),
              do: Answers.rejected(id, "exists"),
              else: add(id, actor_id, file_id, community_id, link_id, event["made_at"])

          removed ->
            add(id, actor_id, file_id, community_id, removed.id, event["made_at"])
        end
    end
  end

  defp decide(actor_id, %{"type" => "update"} = event) do
    %{"id" => id, "row_id" => link_id, "changes" => changes} = event
    row = AppFilesCommunities.get(link_id)

    cond do
      reason = Merge.not_editable(changes, @update_fields) ->
        Answers.rejected(id, reason, server_row(row))

      is_nil(row) ->
        Answers.rejected(id, "unknown_row")

      is_nil(AppUsersCommunities.get_live(actor_id, row.community_id)) ->
        Answers.rejected(id, "not_a_member")

      true ->
        case {row.deleted_at, Time.from_wire(Map.get(changes, "deleted_at", Time.to_wire(row.deleted_at)))} do
          {_, :error} -> Answers.rejected(id, "invalid: deleted_at", server_row(row))
          {%DateTime{}, {:ok, nil}} -> add(id, actor_id, row.file_id, row.community_id, row.id, nil)
          {nil, {:ok, %DateTime{} = at}} -> remove(id, actor_id, row.file_id, row.community_id, at)
          _ -> Answers.applied(id, row.version, %{}, server_row(row))
        end
    end
  end

  defp decide(_actor_id, %{"id" => id}), do: Answers.rejected(id, "invalid: type")

  defp add(id, actor_id, file_id, community_id, link_id, made_at) do
    at = Time.to_wire(Time.now())

    command = %AddFileToCommunity{
      file_id: file_id,
      request_id: id,
      actor_id: actor_id,
      link_id: link_id,
      community_id: community_id,
      inserted_at: made_at || at,
      at: at
    }

    dispatched(id, command, link_id)
  end

  defp remove(id, actor_id, file_id, community_id, deleted_at) do
    command = %RemoveFileFromCommunity{
      file_id: file_id,
      request_id: id,
      actor_id: actor_id,
      community_id: community_id,
      deleted_at: Time.to_wire(deleted_at),
      at: Time.to_wire(Time.now())
    }

    link = AppFilesCommunities.get_by_file_and_community(file_id, community_id)
    dispatched(id, command, link && link.id)
  end

  defp dispatched(id, command, link_id) do
    case CommandedApplication.dispatch(command) do
      :ok ->
        row = AppFilesCommunities.get(link_id)

        case AppEvents.get(id) do
          nil -> Answers.applied(id, row.version, %{}, server_row(row))
          recorded -> Answers.from_record(recorded, server_row(row))
        end

      {:error, :no_change} ->
        row = AppFilesCommunities.get(link_id)
        Answers.applied(id, row.version, %{}, server_row(row))

      {:error, :unknown_file} ->
        Answers.rejected(id, "unknown_file", server_row(AppFilesCommunities.get(link_id)))
    end
  end

  @doc "The link as a row on the wire, or nil."
  def server_row(nil), do: nil
  def server_row(row), do: Answers.table_row(AppFilesCommunities.table(), AppFilesCommunities.to_wire(row))

  # Axum's routes

  @doc "Puts a file the actor sees in a community they are in."
  def create(actor_id, file_id, community_id) do
    event = %{
      "id" => Ids.new(),
      "type" => "create",
      "row_id" => Ids.new(),
      "changes" => %{"file_id" => file_id, "community_id" => community_id},
      "made_at" => Time.to_wire(Time.now())
    }

    result(apply_event(actor_id, event))
  end

  @doc "Takes a file the actor sees out of a community they are in."
  def delete(actor_id, file_id, community_id) do
    with %AppFiles{} <- AppFiles.get_visible(actor_id, file_id) || {:error, :not_found},
         %AppFilesCommunities{deleted_at: nil} = row <-
           AppFilesCommunities.get_by_file_and_community(file_id, community_id) || {:error, :not_found} do
      event = %{
        "id" => Ids.new(),
        "type" => "update",
        "row_id" => row.id,
        "base_version" => row.version,
        "base" => %{"deleted_at" => nil},
        "changes" => %{"deleted_at" => Time.to_wire(Time.now())},
        "made_at" => Time.to_wire(Time.now())
      }

      result(apply_event(actor_id, event))
    else
      %AppFilesCommunities{} -> {:error, :not_found}
      error -> error
    end
  end

  @doc "Makes a file's communities among the actor's exactly these. Rows in communities the actor is not in stay."
  def set(actor_id, file_id, community_ids) do
    current = AppFilesCommunities.list_visible_of_files(actor_id, [file_id])

    results =
      Enum.map(community_ids -- Enum.map(current, & &1.community_id), &create(actor_id, file_id, &1)) ++
        for(row <- current, row.community_id not in community_ids, do: delete(actor_id, file_id, row.community_id))

    case Enum.find(results, &match?({:error, _}, &1)) do
      nil -> :ok
      error -> error
    end
  end

  defp result(%{"status" => status, "server" => %{"row" => row}}) when status in ["applied", "conflict"], do: {:ok, row}
  defp result(%{"reason" => "not_a_member"}), do: {:error, :not_a_member}
  defp result(%{"reason" => reason}) when reason in ["unknown_file", "unknown_row"], do: {:error, :not_found}
  defp result(%{"reason" => reason}), do: {:error, {:refused, reason}}

  defp uuid?(value), do: is_binary(value) and match?({:ok, _}, Ecto.UUID.cast(value))
end

defmodule Core.App.Files.Projector do
  @moduledoc """
  Writes the rows a device mirrors from a file's facts, `app_files` and
  `app_files_communities`. Each fact's rows, their global sync versions
  and the answer to the request it decided commit in one transaction,
  with this projector's checkpoint. Strong consistency lets a push
  answer from the rows it just wrote.
  """
  use Commanded.Projections.Ecto,
    application: Core.CommandedApplication,
    name: "files_projector",
    consistency: :strong

  alias Core.App.Files.Events.{
    FileAddedToCommunity,
    FileCreated,
    FileDeleted,
    FileRemovedFromCommunity,
    FileUpdated
  }

  alias Core.Data.{AppEvents, AppFiles, AppFilesCommunities}
  alias Core.Helpers.Time

  project(%FileCreated{} = event, metadata, fn multi ->
    Ecto.Multi.run(multi, :rows, fn _repo, _changes ->
      at = time!(event.at)

      file =
        AppFiles.create(%AppFiles{
          id: event.file_id,
          created_by_id: event.actor_id,
          name: event.name,
          content_type: event.content_type,
          size: event.size,
          sha256: event.sha256,
          content: event.content,
          inserted_at: time!(event.inserted_at),
          updated_at: at,
          version: 1
        })

      for link <- event.links do
        AppFilesCommunities.create(%AppFilesCommunities{
          id: link_field(link, :id),
          file_id: file.id,
          community_id: link_field(link, :community_id),
          created_by_id: event.actor_id,
          inserted_at: at,
          updated_at: at,
          version: 1
        })
      end

      answered(event, metadata, AppFiles.table(), file.id, 1, %{})
    end)
  end)

  project(%FileUpdated{} = event, metadata, fn multi ->
    Ecto.Multi.run(multi, :rows, fn _repo, _changes ->
      at = time!(event.at)
      file = AppFiles.get(event.file_id)
      changes = Map.new(event.changes, fn {field, value} -> {to_string(field), value} end)

      AppFiles.update(%{
        file
        | name: Map.get(changes, "name", file.name),
          version: event.version,
          updated_at: at
      })

      answered(event, metadata, AppFiles.table(), file.id, event.version, event.conflicts)
    end)
  end)

  project(%FileAddedToCommunity{} = event, metadata, fn multi ->
    Ecto.Multi.run(multi, :rows, fn _repo, _changes ->
      at = time!(event.at)
      stored = AppFilesCommunities.get(event.link_id)

      row = %{
        (stored || %AppFilesCommunities{id: event.link_id, file_id: event.file_id, community_id: event.community_id})
        | created_by_id: event.actor_id,
          inserted_at: time!(event.inserted_at),
          updated_at: at,
          version: event.link_version,
          deleted_at: nil,
          deleted_by_id: nil
      }

      AppFilesCommunities.update(row)
      answered(event, metadata, AppFilesCommunities.table(), event.link_id, event.link_version, %{})
    end)
  end)

  project(%FileRemovedFromCommunity{} = event, metadata, fn multi ->
    Ecto.Multi.run(multi, :rows, fn _repo, _changes ->
      at = time!(event.at)
      row = AppFilesCommunities.get(event.link_id)

      AppFilesCommunities.update(%{
        row
        | version: event.link_version,
          updated_at: at,
          deleted_at: time!(event.deleted_at),
          deleted_by_id: event.actor_id
      })

      answered(event, metadata, AppFilesCommunities.table(), event.link_id, event.link_version, %{})
    end)
  end)

  project(%FileDeleted{} = event, _metadata, fn multi ->
    Ecto.Multi.run(multi, :rows, fn _repo, _changes ->
      at = time!(event.at)
      file = AppFiles.get(event.file_id)

      AppFiles.update(%{
        file
        | version: event.version,
          updated_at: at,
          deleted_at: time!(event.deleted_at),
          deleted_by_id: event.actor_id
      })

      {:ok, :deleted}
    end)
  end)

  # The request the fact decided, with its answer, for a retry
  defp answered(event, metadata, table, row_id, version, conflicts) do
    status = if conflicts == %{}, do: "applied", else: "conflict"
    at = time!(event.at)

    made_at =
      case Time.from_wire(metadata["made_at"]) do
        {:ok, %DateTime{} = made_at} -> made_at
        _ -> at
      end

    AppEvents.create(%AppEvents{
      id: event.request_id,
      user_id: event.actor_id,
      table_name: table,
      row_id: row_id,
      type: metadata["type"] || "update",
      base_version: metadata["base_version"] || 0,
      base: metadata["base"] || %{},
      changes: metadata["changes"] || %{},
      made_at: made_at,
      status: status,
      conflicts: conflicts,
      version: version
    })

    {:ok, status}
  end

  defp time!(text) do
    {:ok, time} = Time.from_wire(text)
    time
  end

  defp link_field(link, key), do: Map.get(link, key) || Map.get(link, Atom.to_string(key))
end

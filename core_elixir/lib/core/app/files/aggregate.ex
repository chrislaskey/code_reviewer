defmodule Core.App.Files.Aggregate do
  @moduledoc """
  A file and its communities, one stream per file: the decisions about a
  file serialize here. Authorization and validation happen before a
  command is dispatched, see `Core.App.Files`. The aggregate keeps
  the ids of the requests it decided, so a retried request changes
  nothing.
  """
  alias Core.App.Files.Commands.{AddFileToCommunity, CreateFile, RemoveFileFromCommunity, UpdateFile}

  alias Core.App.Files.Events.{
    FileAddedToCommunity,
    FileCreated,
    FileDeleted,
    FileRemovedFromCommunity,
    FileUpdated
  }

  alias Core.Sync.Merge

  @updatable ~w(name)

  defstruct id: nil, version: 0, fields: %{}, deleted_at: nil, links: %{}, request_ids: MapSet.new()

  # Decisions

  def execute(%__MODULE__{} = file, %{request_id: request_id} = command) do
    if MapSet.member?(file.request_ids, request_id), do: [], else: decide(file, command)
  end

  defp decide(%__MODULE__{id: nil}, %CreateFile{} = command) do
    %FileCreated{
      file_id: command.file_id,
      request_id: command.request_id,
      actor_id: command.actor_id,
      name: command.name,
      content_type: command.content_type,
      size: command.size,
      sha256: command.sha256,
      content: command.content,
      inserted_at: command.inserted_at,
      at: command.at,
      links: command.links
    }
  end

  defp decide(%__MODULE__{}, %CreateFile{}), do: {:error, :exists}
  defp decide(%__MODULE__{id: nil}, _command), do: {:error, :unknown_file}
  defp decide(%__MODULE__{deleted_at: deleted_at}, _command) when not is_nil(deleted_at), do: {:error, :unknown_file}

  defp decide(%__MODULE__{} = file, %UpdateFile{} = command) do
    current = Map.take(file.fields, @updatable)
    {applied, conflicts} = Merge.merge(current, file.version, command.base_version, command.base, command.changes)

    if applied == %{} do
      {:error, {:no_change, conflicts}}
    else
      %FileUpdated{
        file_id: file.id,
        request_id: command.request_id,
        actor_id: command.actor_id,
        version: file.version + 1,
        at: command.at,
        changes: applied,
        conflicts: conflicts
      }
    end
  end

  defp decide(%__MODULE__{} = file, %AddFileToCommunity{} = command) do
    case Map.get(file.links, command.community_id) do
      %{deleted_at: nil} ->
        {:error, :no_change}

      link ->
        %FileAddedToCommunity{
          file_id: file.id,
          request_id: command.request_id,
          actor_id: command.actor_id,
          link_id: (link && link.id) || command.link_id,
          community_id: command.community_id,
          link_version: if(link, do: link.version + 1, else: 1),
          inserted_at: (link && link.inserted_at) || command.inserted_at,
          at: command.at
        }
    end
  end

  defp decide(%__MODULE__{} = file, %RemoveFileFromCommunity{} = command) do
    case Map.get(file.links, command.community_id) do
      %{deleted_at: nil} = link ->
        removed = %FileRemovedFromCommunity{
          file_id: file.id,
          request_id: command.request_id,
          actor_id: command.actor_id,
          link_id: link.id,
          community_id: command.community_id,
          link_version: link.version + 1,
          deleted_at: command.deleted_at,
          at: command.at
        }

        live_after =
          Enum.count(file.links, fn {id, other} -> id != command.community_id and is_nil(other.deleted_at) end)

        if live_after == 0 do
          [
            removed,
            %FileDeleted{
              file_id: file.id,
              request_id: command.request_id,
              actor_id: command.actor_id,
              version: file.version + 1,
              deleted_at: command.deleted_at,
              at: command.at
            }
          ]
        else
          removed
        end

      _ ->
        {:error, :no_change}
    end
  end

  # State

  def apply(%__MODULE__{} = file, %FileCreated{} = event) do
    links =
      Map.new(event.links, fn link ->
        community_id = link_field(link, :community_id)
        {community_id, %{id: link_field(link, :id), version: 1, deleted_at: nil, inserted_at: event.at}}
      end)

    %__MODULE__{
      file
      | id: event.file_id,
        version: 1,
        fields: %{"name" => event.name},
        links: links,
        request_ids: MapSet.put(file.request_ids, event.request_id)
    }
  end

  def apply(%__MODULE__{} = file, %FileUpdated{} = event) do
    %__MODULE__{
      file
      | version: event.version,
        fields: Map.merge(file.fields, stringify(event.changes)),
        request_ids: MapSet.put(file.request_ids, event.request_id)
    }
  end

  def apply(%__MODULE__{} = file, %FileAddedToCommunity{} = event) do
    link = %{id: event.link_id, version: event.link_version, deleted_at: nil, inserted_at: event.inserted_at}

    %__MODULE__{
      file
      | links: Map.put(file.links, event.community_id, link),
        request_ids: MapSet.put(file.request_ids, event.request_id)
    }
  end

  def apply(%__MODULE__{} = file, %FileRemovedFromCommunity{} = event) do
    links =
      Map.update!(file.links, event.community_id, fn link ->
        %{link | version: event.link_version, deleted_at: event.deleted_at}
      end)

    %__MODULE__{file | links: links, request_ids: MapSet.put(file.request_ids, event.request_id)}
  end

  def apply(%__MODULE__{} = file, %FileDeleted{} = event) do
    %__MODULE__{file | version: event.version, deleted_at: event.deleted_at}
  end

  # Links arrive with string keys from a command and atom keys from the event store
  defp link_field(link, key), do: Map.get(link, key) || Map.get(link, Atom.to_string(key))

  defp stringify(map), do: Map.new(map, fn {key, value} -> {to_string(key), value} end)
end

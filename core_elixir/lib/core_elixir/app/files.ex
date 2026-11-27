defmodule CoreElixir.App.Files do
  @moduledoc """
  Files: what Axum's file routes and a pushed `app_files` event do. A
  decision about a file is a command to its aggregate,
  `CoreElixir.App.Files.Aggregate`, whose facts the projector writes as
  rows. What needs other resources, such as membership, is checked here
  first, from their current rows.
  """
  alias CoreElixir.App.Files.Commands.{CreateFile, UpdateFile}
  alias CoreElixir.App.FilesCommunities
  alias CoreElixir.CommandedApplication
  alias CoreElixir.Data.{AppCommunities, AppEvents, AppFiles, AppFilesCommunities, AppUsers, AppUsersCommunities}
  alias CoreElixir.Helpers.{Ids, Time}
  alias CoreElixir.Sync.{Answers, Merge}

  @create_fields ~w(name content_type size sha256 content communities)
  @update_fields ~w(name)
  @max_list_limit 500

  # Pushed events

  @doc """
  Applies a pushed event of the actor's to its file and answers it, as
  `files::apply_event` does. `uploaded` says whether Rust found the
  create's bytes in the bucket. A retried event gets the answer it got.
  """
  def apply_event(actor_id, %{"id" => id} = event, uploaded) do
    case AppEvents.get(id) do
      nil -> decide(actor_id, event, uploaded)
      recorded -> Answers.from_record(recorded, server_row(AppFiles.get(recorded.row_id)))
    end
  end

  defp decide(actor_id, %{"type" => "create"} = event, uploaded) do
    %{"id" => id, "row_id" => file_id, "changes" => changes} = event
    fields = Map.take(changes, ~w(name content_type size sha256 content))

    with nil <- not_editable(id, changes, @create_fields),
         nil <- existing(id, actor_id, file_id),
         nil <- invalid(id, AppFiles.field_errors(fields)),
         {:ok, links} <- links(id, changes["communities"]),
         nil <- FilesCommunities.refusal_of_new(id, actor_id, links),
         nil <- not_uploaded(id, fields, uploaded) do
      at = Time.to_wire(Time.now())

      command = %CreateFile{
        file_id: file_id,
        request_id: id,
        actor_id: actor_id,
        name: fields["name"],
        content_type: fields["content_type"],
        size: fields["size"],
        sha256: fields["sha256"],
        content: fields["content"],
        inserted_at: inserted_at(event, at),
        at: at,
        links: links
      }

      case CommandedApplication.dispatch(command) do
        :ok -> recorded_answer(id, AppFiles.get(file_id))
        {:error, :exists} -> Answers.rejected(id, "exists", server_row(AppFiles.get_visible(actor_id, file_id)))
      end
    else
      {:answer, answer} -> answer
      answer when is_map(answer) -> answer
    end
  end

  defp decide(actor_id, %{"type" => "update"} = event, _uploaded) do
    %{"id" => id, "row_id" => file_id, "changes" => changes} = event

    case AppFiles.get_visible(actor_id, file_id) do
      nil ->
        Answers.rejected(id, "unknown_file")

      file ->
        with nil <- not_editable(id, changes, @update_fields, file),
             nil <- invalid(id, name_errors(changes), file) do
          command = %UpdateFile{
            file_id: file_id,
            request_id: id,
            actor_id: actor_id,
            base_version: event["base_version"],
            base: event["base"] || %{},
            changes: changes,
            at: Time.to_wire(Time.now())
          }

          case CommandedApplication.dispatch(command) do
            :ok ->
              recorded_answer(id, AppFiles.get(file_id))

            {:error, {:no_change, conflicts}} ->
              Answers.applied(id, file.version, conflicts, server_row(file))

            {:error, :unknown_file} ->
              Answers.rejected(id, "unknown_file")
          end
        end
    end
  end

  defp decide(_actor_id, %{"id" => id}, _uploaded), do: Answers.rejected(id, "invalid: type")

  # The answer the projector recorded with the rows, read back now that
  # they are written; a retry the aggregate had already decided finds it too
  defp recorded_answer(id, file) do
    case AppEvents.get(id) do
      nil -> Answers.applied(id, file.version, %{}, server_row(file))
      recorded -> Answers.from_record(recorded, server_row(file))
    end
  end

  defp not_editable(id, changes, allowed, file \\ nil) do
    case Merge.not_editable(changes, allowed) do
      nil -> nil
      reason -> Answers.rejected(id, reason, server_row(file))
    end
  end

  defp existing(id, actor_id, file_id) do
    if AppFiles.get(file_id), do: Answers.rejected(id, "exists", server_row(AppFiles.get_visible(actor_id, file_id)))
  end

  defp invalid(id, errors, file \\ nil)
  defp invalid(_id, errors, _file) when errors == %{}, do: nil
  defp invalid(id, errors, file), do: Answers.rejected(id, Answers.invalid(errors), server_row(file))

  defp name_errors(%{"name" => name}) do
    case AppFiles.name_error(name) do
      nil -> %{}
      error -> %{"name" => [error]}
    end
  end

  defp name_errors(_changes), do: %{}

  defp links(_id, nil), do: {:ok, []}

  defp links(id, links) when is_list(links) do
    if Enum.all?(links, &(is_map(&1) and uuid?(&1["id"]) and uuid?(&1["community_id"]))),
      do: {:ok, Enum.map(links, &Map.take(&1, ~w(id community_id)))},
      else: {:answer, Answers.rejected(id, "invalid: communities")}
  end

  defp links(id, _links), do: {:answer, Answers.rejected(id, "invalid: communities")}

  defp not_uploaded(id, fields, uploaded) do
    if fields["content"] || uploaded, do: nil, else: Answers.rejected(id, "file_not_uploaded")
  end

  defp inserted_at(event, at) do
    case Time.from_wire(event["made_at"]) do
      {:ok, %DateTime{} = made_at} -> Time.to_wire(made_at)
      _ -> at
    end
  end

  defp uuid?(value), do: is_binary(value) and match?({:ok, _}, Ecto.UUID.cast(value))

  @doc "The file as a row on the wire, or nil."
  def server_row(nil), do: nil
  def server_row(file), do: Answers.table_row(AppFiles.table(), AppFiles.to_wire(file))

  # Axum's routes

  @doc "The live files the actor sees that pass the filter, a page of them newest first, with how many pass in all."
  def list(actor_id, filter) do
    limit = filter["limit"] || 0

    if limit > @max_list_limit do
      {:error, {:invalid, %{"limit" => ["at most #{@max_list_limit} files at once"]}}}
    else
      {total, files} =
        AppFiles.search(actor_id, filter["name"] || "", filter["content_types"] || [], filter["offset"] || 0, limit)

      {:ok, %{"total" => total, "files" => with_states(actor_id, files)}}
    end
  end

  @doc "One live file the actor sees, with its state, creator and communities."
  def get(actor_id, id) do
    case AppFiles.get_visible(actor_id, id) do
      nil -> {:error, :not_found}
      file -> {:ok, hd(with_states(actor_id, [file]))}
    end
  end

  @doc "Creates a file from the fields of a `FileNew`, in the actor's communities it names."
  def create(actor_id, new, uploaded) do
    communities = Enum.map(new["community_ids"] || [], &%{"id" => Ids.new(), "community_id" => &1})

    changes =
      new
      |> Map.take(~w(name content_type size sha256 content))
      |> Map.reject(fn {field, value} -> field == "content" and is_nil(value) end)
      |> Map.put("communities", communities)

    event = %{
      "id" => Ids.new(),
      "type" => "create",
      "row_id" => new["id"],
      "changes" => changes,
      "made_at" => Time.to_wire(Time.now())
    }

    answer_to_result(apply_event(actor_id, event, uploaded))
  end

  @doc "Renames a file the actor sees, and makes its communities among the actor's exactly these."
  def update(actor_id, id, name, community_ids) do
    with %AppFiles{} = file <- AppFiles.get_visible(actor_id, id) || {:error, :not_found},
         {:ok, _} <- rename(actor_id, file, name),
         :ok <- FilesCommunities.set(actor_id, id, community_ids) do
      {:ok, AppFiles.to_wire(AppFiles.get(id))}
    end
  end

  defp rename(_actor_id, %AppFiles{name: name} = file, name), do: {:ok, file}

  defp rename(actor_id, file, name) do
    event = %{
      "id" => Ids.new(),
      "type" => "update",
      "row_id" => file.id,
      "base_version" => file.version,
      "base" => %{"name" => file.name},
      "changes" => %{"name" => name},
      "made_at" => Time.to_wire(Time.now())
    }

    answer_to_result(apply_event(actor_id, event, true))
  end

  @doc "Takes a file the actor sees out of every community of theirs; it is deleted when those were its last."
  def delete(actor_id, id) do
    case AppFiles.get_visible(actor_id, id) do
      nil ->
        {:error, :not_found}

      _file ->
        for row <- AppFilesCommunities.list_visible_of_files(actor_id, [id]) do
          FilesCommunities.delete(actor_id, id, row.community_id)
        end

        :ok
    end
  end

  @doc "What Rust needs to sign a download of a file the actor sees, or to serve the content its row keeps."
  def readable(actor_id, id) do
    case AppFiles.get_visible(actor_id, id) do
      nil ->
        {:error, :not_found}

      file ->
        {:ok,
         %{
           "name" => file.name,
           "content_type" => file.content_type,
           "storage_key" => AppFiles.storage_key(file),
           "content" => file.content
         }}
    end
  end

  @doc "The storage keys of every file row, deleted or not, for Rust's bucket sweep."
  def storage_keys_list, do: AppFiles.list_storage_keys()

  @doc "The live files the actor sees with any of the checksums."
  def duplicates_list(actor_id, sha256s),
    do: Enum.map(AppFiles.list_with_sha256s(actor_id, sha256s), &AppFiles.to_wire/1)

  defp answer_to_result(%{"status" => status, "server" => %{"row" => row}}) when status in ["applied", "conflict"],
    do: {:ok, row}

  defp answer_to_result(%{"reason" => "not_a_member"}), do: {:error, :not_a_member}
  defp answer_to_result(%{"reason" => "exists"}), do: {:error, :exists}
  defp answer_to_result(%{"reason" => "unknown_file"}), do: {:error, :not_found}
  defp answer_to_result(%{"reason" => reason}), do: {:error, {:refused, reason}}

  # A file as the pages show it: its creator while they are a live member
  # of one of its communities that the actor is in, and those communities
  defp with_states(_actor_id, []), do: []

  defp with_states(actor_id, files) do
    rows = AppFilesCommunities.list_visible_of_files(actor_id, Enum.map(files, & &1.id))

    communities =
      rows |> Enum.map(& &1.community_id) |> Enum.uniq() |> AppCommunities.list_by_ids() |> Map.new(&{&1.id, &1})

    creators = files |> Enum.map(& &1.created_by_id) |> Enum.uniq() |> AppUsers.list_by_ids() |> Map.new(&{&1.id, &1})

    Enum.map(files, fn file ->
      community_ids = for row <- rows, row.file_id == file.id, do: row.community_id

      creator =
        if Enum.any?(community_ids, &AppUsersCommunities.get_live(file.created_by_id, &1)),
          do: creators[file.created_by_id] && AppUsers.to_wire(creators[file.created_by_id])

      %{
        "file" => AppFiles.to_wire(file),
        "creator" => creator,
        "communities" =>
          community_ids |> Enum.map(&communities[&1]) |> Enum.reject(&is_nil/1) |> Enum.map(&AppCommunities.to_wire/1),
        "shown_as" => shown_as(file.content_type),
        "state" => "synced",
        "error" => nil
      }
    end)
  end

  defp shown_as(content_type) do
    case content_type |> String.split(";") |> hd() |> String.trim() do
      "text/plain" -> "text"
      "application/pdf" -> "pdf"
      "image/" <> _ -> "image"
      _ -> "not_shown"
    end
  end
end

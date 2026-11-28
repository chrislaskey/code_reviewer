defmodule Core.App.Users do
  @moduledoc "Users: who a token stands for, the user a sign-up makes, and a person renaming themselves."
  alias Core.Data.{AccountsTokens, AccountsUsers, AppEvents, AppSyncVersions, AppUsers, SyncedRow}
  alias Core.Helpers.Time
  alias Core.Repo
  alias Core.Sync.{Answers, Events, Merge}

  @update_fields ~w(name)

  @doc "The user a token stands for, with their address and kind, as `users::get_by_token` answers it."
  def get_by_token(token) when is_binary(token) do
    with %AccountsTokens{} = row <-
           AccountsTokens.get_valid(token, AccountsTokens.context_api()) || {:error, :unknown_token},
         %AccountsUsers{deleted_at: nil} = account <- AccountsUsers.get(row.user_id) || {:error, :unknown_token},
         %AppUsers{deleted_at: nil} = user <- AppUsers.get(row.user_id) || {:error, :unknown_token} do
      kind = if account.trial_expires_at, do: "trial", else: "account"
      {:ok, %{"user" => AppUsers.to_wire(user), "email" => account.email, "kind" => kind}}
    else
      %AccountsUsers{} -> {:error, :account_deleted}
      %AppUsers{} -> {:error, :account_deleted}
      error -> error
    end
  end

  def get_by_token(_token), do: {:error, :unknown_token}

  @doc "Creates a user's row with the name, and publishes it. In the caller's transaction."
  def create_in(id, name) do
    now = Time.now()
    AppUsers.create(%AppUsers{id: id, name: name, inserted_at: now, updated_at: now, version: 1})
  end

  @doc """
  A name from the address, before its `@`, that no other live user has,
  with a number after it when one has, as `users::name_from_email_in`
  makes it.
  """
  def name_from_email_in(email, id) do
    before_at = email |> String.split("@") |> hd()

    Stream.iterate(0, &(&1 + 1))
    |> Stream.map(fn number ->
      suffix = if number == 0, do: "", else: Integer.to_string(number)
      String.slice(before_at, 0, AppUsers.max_name_length() - String.length(suffix)) <> suffix
    end)
    |> Enum.find(&(not AppUsers.exists_by_name?(&1, id)))
  end

  @doc """
  Writes a new name over the user's own row, as `users::update` does: an
  event like any change a person makes. Answers the user.
  """
  def update(user_id, name) do
    case checked_name(name) do
      nil ->
        {:error, {:invalid, %{"name" => ["is needed, #{AppUsers.max_name_length()} characters at most"]}}}

      name ->
        Repo.transaction(fn ->
          AppSyncVersions.lock()

          case AppUsers.get(user_id) do
            %AppUsers{deleted_at: nil, name: ^name} = user ->
              AppUsers.to_wire(user)

            %AppUsers{deleted_at: nil} = user ->
              event = Events.update(AppUsers.table(), user.id, user.version, %{"name" => user.name}, %{"name" => name})

              case apply_in(user_id, event) do
                %{"status" => "rejected", "reason" => reason} -> Repo.rollback({:refused, reason})
                _applied -> AppUsers.to_wire(AppUsers.get(user_id))
              end

            _ ->
              Repo.rollback(:not_found)
          end
        end)
    end
  end

  @doc "Applies a pushed event of the actor's to their own row and answers it. A retried event gets the answer it got."
  def apply_event(actor_id, event) do
    Repo.transaction(fn ->
      case AppEvents.get(event["id"]) do
        nil ->
          AppSyncVersions.lock()
          apply_in(actor_id, event)

        recorded ->
          Answers.from_record(recorded, server_row(AppUsers.get(recorded.row_id)))
      end
    end)
  end

  # An update of the user's own name merges into the row as a file's
  # does. Only the server makes a user, so a create is refused, and so
  # is a change to anyone else.
  defp apply_in(actor_id, %{"id" => id, "row_id" => row_id, "changes" => changes} = event) do
    row = AppUsers.get(row_id)

    cond do
      is_nil(row) ->
        Answers.rejected(id, "unknown_user")

      event["type"] != "update" ->
        Answers.rejected(id, "server_only", server_row(row))

      row.id != actor_id ->
        Answers.rejected(id, "not_yours", server_row(row))

      reason = Merge.not_editable(changes, @update_fields) ->
        Answers.rejected(id, reason, server_row(row))

      true ->
        {applied, conflicts} =
          Merge.merge(%{"name" => row.name}, row.version, event["base_version"], event["base"] || %{}, changes)

        case Map.fetch(applied, "name") do
          :error ->
            finish(actor_id, event, conflicts, row)

          {:ok, name} ->
            case checked_name(name) do
              nil ->
                Answers.rejected(id, "invalid: a name is needed", server_row(row))

              name ->
                updated = AppUsers.update(%{SyncedRow.changed(row, Time.now()) | name: name})
                finish(actor_id, event, conflicts, updated)
            end
        end
    end
  end

  defp finish(actor_id, event, conflicts, row) do
    answer = Answers.applied(event["id"], row.version, conflicts, server_row(row))
    Events.record(actor_id, event, answer)
    answer
  end

  defp server_row(nil), do: nil
  defp server_row(row), do: Answers.table_row(AppUsers.table(), AppUsers.to_wire(row))

  # A name as it is kept, trimmed, or nil when it is empty or too long
  defp checked_name(name) when is_binary(name) do
    name = String.trim(name)
    if name != "" and String.length(name) <= AppUsers.max_name_length(), do: name
  end

  defp checked_name(_name), do: nil

  @doc "Sets the user's name, a server write the devices pull, for a trial user signing up. In the caller's transaction."
  def update_name_in(id, name) do
    user = AppUsers.get(id)

    if user.name == name,
      do: user,
      else: AppUsers.update(%{SyncedRow.changed(user, Time.now()) | name: name})
  end
end

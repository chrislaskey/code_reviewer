defmodule Core.Accounts.Preferences do
  @moduledoc """
  What a person chooses for themselves, as `accounts::preferences` in
  the Rust core: the language and the theme the pages are shown in, one
  row per user and key. Each write is an event, and the last one the
  server applies wins: a preference has no conflicts. The languages and
  themes a person may choose are the Rust core's lists, read from its
  source when this module compiles.
  """
  alias Core.Data.{AccountsPreferences, AppEvents, AppSyncVersions}
  alias Core.Helpers.{Ids, RustConstants, Time}
  alias Core.Repo
  alias Core.Sync.{Answers, Events, Merge}

  @locales_file Path.expand("../../../../core/src/app/locales.rs", __DIR__)
  @themes_file Path.expand("../../../../core/src/helpers/themes.rs", __DIR__)
  @external_resource @locales_file
  @external_resource @themes_file
  @locales RustConstants.strings!(@locales_file, "SUPPORTED")
  @themes RustConstants.strings!(@themes_file, "SUPPORTED")
  @default_locale hd(@locales)

  @create_fields ~w(user_id key value)
  @update_fields ~w(value)

  @doc "The user's preferences, each nil when they made no choice, or made one that no longer applies."
  def get(user_id) do
    chosen =
      for row <- AccountsPreferences.list_of_user(user_id), valid?(row.key, row.value), into: %{} do
        {row.key, row.value}
      end

    %{"locale" => chosen["locale"], "theme" => chosen["theme"]}
  end

  @doc "The language to mail the user in: the one they chose, else `requested`, if the app has it, else English."
  def mail_locale(user_id, requested) do
    chosen =
      case user_id && AccountsPreferences.get_by_user_and_key(user_id, "locale") do
        %AccountsPreferences{value: locale} when is_binary(locale) -> locale
        _ -> nil
      end

    locale = chosen || requested
    if locale in @locales, do: locale, else: @default_locale
  end

  @doc """
  Sets one of the user's preferences, as `preferences::update` does: an
  event like any change a person makes. Answers the preferences.
  """
  def update(user_id, key, value) do
    with nil <- invalid(key, value) do
      Repo.transaction(fn ->
        AppSyncVersions.lock()
        table = AccountsPreferences.table()

        case AccountsPreferences.get_by_user_and_key(user_id, key) do
          %AccountsPreferences{value: ^value} ->
            get(user_id)

          nil ->
            Events.create(table, Ids.new(), %{"user_id" => user_id, "key" => key, "value" => value})
            |> applied(user_id)

          row ->
            Events.update(table, row.id, row.version, %{"value" => row.value}, %{"value" => value})
            |> applied(user_id)
        end
      end)
    else
      reason -> {:error, {:refused, reason}}
    end
  end

  defp applied(event, user_id) do
    case apply_in(user_id, event) do
      %{"status" => "rejected", "reason" => reason} -> Repo.rollback({:refused, reason})
      _applied -> get(user_id)
    end
  end

  @doc "Applies a pushed event of the actor's to their preference and answers it. A retried event gets the answer it got."
  def apply_event(actor_id, event) do
    Repo.transaction(fn ->
      case AppEvents.get(event["id"]) do
        nil ->
          AppSyncVersions.lock()
          apply_in(actor_id, event)

        recorded ->
          Answers.from_record(recorded, server_row(AccountsPreferences.get(recorded.row_id)))
      end
    end)
  end

  # A create for a key the user already has writes its value over that
  # row, which the answer carries, so the device's row under the other id
  # goes. Only the user's own rows change, and only to a value their key
  # takes.
  defp apply_in(actor_id, %{"id" => id, "row_id" => row_id, "changes" => changes} = event) do
    allowed = if event["type"] == "create", do: @create_fields, else: @update_fields

    with nil <- refused(id, Merge.not_editable(changes, allowed), AccountsPreferences.get(row_id)),
         nil <- refused(id, unless(Map.has_key?(changes, "value"), do: "invalid: a value is needed"), nil),
         {:ok, row} <- row_to_change(actor_id, event) do
      value = changes["value"]

      cond do
        row.value == value ->
          finish(actor_id, event, row)

        reason = invalid(row.key, value) ->
          Answers.rejected(id, reason, server_row(row))

        true ->
          updated = %{row | value: value, version: row.version + 1, updated_at: Time.now()}
          finish(actor_id, event, AccountsPreferences.update(updated))
      end
    else
      {:create, key, value} -> create(actor_id, event, key, value)
      answer -> answer
    end
  end

  defp row_to_change(actor_id, %{"type" => "create", "id" => id, "row_id" => row_id, "changes" => changes}) do
    key = changes["key"]

    cond do
      changes["user_id"] != actor_id ->
        Answers.rejected(id, "not_yours")

      not is_binary(key) ->
        Answers.rejected(id, "invalid: a key is needed")

      existing = AccountsPreferences.get_by_user_and_key(actor_id, key) ->
        {:ok, existing}

      other = AccountsPreferences.get(row_id) ->
        Answers.rejected(id, "exists", server_row(other))

      true ->
        {:create, key, changes["value"]}
    end
  end

  defp row_to_change(actor_id, %{"id" => id, "row_id" => row_id}) do
    case AccountsPreferences.get(row_id) do
      nil -> Answers.rejected(id, "unknown_preference")
      %AccountsPreferences{user_id: ^actor_id} = row -> {:ok, row}
      _other -> Answers.rejected(id, "not_yours")
    end
  end

  defp create(actor_id, event, key, value) do
    case invalid(key, value) do
      nil ->
        now = Time.now()

        preference =
          AccountsPreferences.create(%AccountsPreferences{
            id: event["row_id"],
            user_id: actor_id,
            key: key,
            value: value,
            inserted_at: inserted_at(event["made_at"], now),
            updated_at: now,
            version: 1
          })

        finish(actor_id, event, preference)

      reason ->
        Answers.rejected(event["id"], reason)
    end
  end

  # The device's time it was made, unless its clock is ahead
  defp inserted_at(made_at, now) do
    case Time.from_wire(made_at) do
      {:ok, %DateTime{} = made_at} -> Enum.min([made_at, now], DateTime)
      _ -> now
    end
  end

  defp finish(actor_id, event, row) do
    answer = Answers.applied(event["id"], row.version, %{}, server_row(row))
    Events.record(actor_id, event, answer)
    answer
  end

  defp refused(_id, nil, _row), do: nil
  defp refused(id, reason, row), do: Answers.rejected(id, reason, server_row(row))

  defp server_row(nil), do: nil
  defp server_row(row), do: Answers.table_row(AccountsPreferences.table(), AccountsPreferences.to_wire(row))

  # The reason a value is not one the key takes, as `PreferenceInvalid` prints it, or nil
  defp invalid(key, value) when key in ["locale", "theme"] do
    if valid?(key, value), do: nil, else: "invalid: not a value for #{key}: #{Jason.encode!(value)}"
  end

  defp invalid(key, _value), do: "invalid: unknown preference: #{key}"

  defp valid?(_key, nil), do: true
  defp valid?("locale", locale), do: locale in @locales
  defp valid?("theme", theme), do: theme in @themes
  defp valid?(_key, _value), do: false
end

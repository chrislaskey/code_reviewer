defmodule Core.Sync.Events do
  @moduledoc """
  The events the server makes for a person's change, as `Event::create`
  and `Event::update` in the Rust core, and their record in
  `app_events`, as `events::record` writes it: refused for a trial user
  who has made all the writes a trial may.
  """
  alias Core.Accounts.TrialUsers
  alias Core.Data.AppEvents
  alias Core.Helpers.{Ids, Time}
  alias Core.Repo

  @doc "A create of the row with the fields, made now."
  def create(table, row_id, changes) do
    %{
      "id" => Ids.new(),
      "table" => table,
      "row_id" => row_id,
      "type" => "create",
      "base_version" => 0,
      "base" => %{},
      "changes" => changes,
      "made_at" => Time.to_wire(Time.now())
    }
  end

  @doc "An update of the row from the version and fields it had, made now."
  def update(table, row_id, base_version, base, changes) do
    %{
      "id" => Ids.new(),
      "table" => table,
      "row_id" => row_id,
      "type" => "update",
      "base_version" => base_version,
      "base" => base,
      "changes" => changes,
      "made_at" => Time.to_wire(Time.now())
    }
  end

  @doc """
  Records the event of the user's with its answer, in the caller's
  transaction. A trial user's past `TrialUsers.max_writes/0` rolls the
  transaction back with `:trial_writes_full`, so nothing is written.
  """
  def record(user_id, event, answer) do
    if TrialUsers.writes_full?(user_id), do: Repo.rollback(:trial_writes_full)
    {:ok, made_at} = Time.from_wire(event["made_at"])

    AppEvents.create(%AppEvents{
      id: event["id"],
      user_id: user_id,
      table_name: event["table"],
      row_id: event["row_id"],
      type: event["type"],
      base_version: event["base_version"],
      base: event["base"],
      changes: event["changes"],
      made_at: made_at,
      status: answer["status"],
      conflicts: answer["conflicts"],
      version: answer["version"],
      last_error: answer["reason"]
    })
  end
end

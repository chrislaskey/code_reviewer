defmodule CoreElixir.Sync.Merge do
  @moduledoc """
  The field-by-field merge of an event into a row, as `sync::events::merge`
  in the Rust core does it.
  """

  @doc """
  Merges `changes` into the row's `current` values at `version`. At the
  server's version each changed value applies. Behind it, a field the
  row already holds the device's value for is skipped; a field still at
  the device's `base` value applies, since nobody else changed it; any
  other field was changed by someone else first, and the device's value
  is a conflict, `%{"yours" => …, "server" => …}`. Returns
  `{applied, conflicts}`.
  """
  def merge(current, version, base_version, _base, changes) when base_version == version do
    applied = for {field, value} <- changes, Map.get(current, field) != value, into: %{}, do: {field, value}
    {applied, %{}}
  end

  def merge(current, _version, _base_version, base, changes) do
    Enum.reduce(changes, {%{}, %{}}, fn {field, value}, {applied, conflicts} ->
      server = Map.get(current, field)

      cond do
        server == value -> {applied, conflicts}
        Map.get(base, field) == server -> {Map.put(applied, field, value), conflicts}
        true -> {applied, Map.put(conflicts, field, %{"yours" => value, "server" => server})}
      end
    end)
  end

  @doc "The reason to refuse an event that changes a field it may not, or nil."
  def not_editable(changes, allowed) do
    case changes |> Map.keys() |> Enum.reject(&(&1 in allowed)) |> Enum.sort() do
      [] -> nil
      refused -> "not_editable: " <> Enum.join(refused, ", ")
    end
  end
end

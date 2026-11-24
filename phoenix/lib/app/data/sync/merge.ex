defmodule App.Data.Sync.Merge do
  @moduledoc """
  The field-level merge of an event into its row, see "Events" in
  `docs/design/offline-sync.md`.
  """

  @doc """
  Merges `changes` into `current`, the row's wire values at `version`.
  Returns `{applied, conflicts}`.

      iex> merge(%{"body" => "b"}, 2, 1, %{"body" => "a"}, %{"body" => "c"})
      {%{}, %{"body" => %{"yours" => "c", "server" => "b"}}}
  """
  def merge(_current, version, version, _base, changes), do: {changes, %{}}

  def merge(current, _version, _base_version, base, changes) do
    Enum.reduce(changes, {%{}, %{}}, fn {field, value}, {applied, conflicts} ->
      server = Map.get(current, field)

      cond do
        server == value ->
          {applied, conflicts}

        Map.has_key?(base, field) and Map.fetch!(base, field) == server ->
          {Map.put(applied, field, value), conflicts}

        true ->
          {applied, Map.put(conflicts, field, %{"yours" => value, "server" => server})}
      end
    end)
  end

  @doc """
  The fields of a row, as wire values keyed by name.
  """
  def values(row, fields), do: Map.new(fields, &{&1, wire(Map.fetch!(row, String.to_existing_atom(&1)))})

  @doc """
  A value as it travels on the wire.
  """
  def wire(%DateTime{} = datetime), do: DateTime.to_iso8601(datetime)
  def wire(value), do: value
end

defmodule CoreElixir.Sync.Tables do
  @moduledoc """
  The synced tables this backend serves, parents before the rows that
  point at them: the order a device writes a page in. Each table's
  context answers `list_synced_after/4`, `to_wire/1` and `table/0`; its
  parents and backfill rows are here.
  """
  alias CoreElixir.Data.{AppCommunities, AppFiles, AppFilesCommunities, AppUsers, AppUsersCommunities}

  @doc "Every table, parents first."
  def list, do: [AppUsers, AppCommunities, AppUsersCommunities, AppFiles, AppFilesCommunities]

  @doc "The rows a row points at, which a pull sends before it, as `{table module, row}`."
  def parents(AppUsersCommunities, membership) do
    [{AppUsers, AppUsers.get(membership.user_id)}, {AppCommunities, AppCommunities.get(membership.community_id)}]
    |> Enum.reject(fn {_table, row} -> is_nil(row) end)
  end

  def parents(AppFilesCommunities, row) do
    file = if is_nil(row.deleted_at), do: AppFiles.get(row.file_id)

    [{AppFiles, file}, {AppCommunities, AppCommunities.get(row.community_id)}]
    |> Enum.reject(fn {_table, parent} -> is_nil(parent) end)
  end

  def parents(_table, _row), do: []

  @doc "The live rows in a community with a version in the range, for a backfill; none for a table not in communities."
  def list_live_in_community_after(AppFilesCommunities, community_id, after_version, through, limit),
    do: AppFilesCommunities.list_live_of_community_after(community_id, after_version, through, limit)

  def list_live_in_community_after(_table, _community_id, _after_version, _through, _limit), do: []
end

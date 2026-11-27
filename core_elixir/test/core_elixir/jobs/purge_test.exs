defmodule CoreElixir.Jobs.PurgeTest do
  use CoreElixir.BackendCase, async: false

  alias CoreElixir.App.Files
  alias CoreElixir.Helpers.{Ids, Time}
  alias CoreElixir.Jobs.Purge
  alias CoreElixir.Sync

  test "purge_moves_the_floor_and_a_cursor_below_it_resyncs" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    {_rows, cursor} = pull_all(alice, nil)
    file_id = Ids.new()
    {:ok, _} = Sync.push(alice, [file_create(file_id, personal_community(alice), "a.txt")], [])
    :ok = Files.delete(alice, file_id)

    # As if the retention window had passed
    {:ok, %{floor: floor, removed: removed}} = Purge.run(DateTime.add(Time.now(), 91, :day))
    assert floor > String.to_integer(cursor["global_sync_version"])
    assert removed == %{"app_files_communities" => 1, "app_files" => 1}
    assert {:ok, %{"resync" => true}} = Sync.pull(alice, %{"cursor" => cursor})

    {_rows, fresh} = pull_all(alice, nil)
    assert {:ok, %{"resync" => false}} = Sync.pull(alice, %{"cursor" => fresh})
  end
end

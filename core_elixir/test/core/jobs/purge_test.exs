defmodule Core.Jobs.PurgeTest do
  use Core.BackendCase, async: false

  alias Core.App.Files
  alias Core.Helpers.{Ids, Time}
  alias Core.Jobs.Purge
  alias Core.Sync

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

  test "purge_removes_tombstones_while_a_writer_holds_the_feed_lock" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    file_id = Ids.new()
    {:ok, _} = Sync.push(alice, [file_create(file_id, personal_community(alice), "a.txt")], [])
    :ok = Files.delete(alice, file_id)
    horizon = DateTime.add(Time.now(), 1, :day)
    %{head: floor} = Core.Data.AppSyncVersions.get()

    parent = self()

    writer =
      Task.async(fn ->
        Core.Repo.transaction(fn ->
          Core.Data.AppSyncVersions.lock()
          send(parent, :locked)

          receive do
            :done -> :ok
          end
        end)
      end)

    assert_receive :locked
    deleting = Task.async(fn -> Purge.delete_tombstones(horizon, floor) end)
    assert %{"app_files_communities" => 1, "app_files" => 1} = Task.await(deleting, 5_000)
    send(writer.pid, :done)
    Task.await(writer)
  end
end

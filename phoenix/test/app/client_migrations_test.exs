defmodule App.ClientMigrationsTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.ClientMigrations
  alias App.Data.Sync
  alias Exqlite.Sqlite3

  @moduletag :tmp_dir

  # Fails when a migration and its exported file have drifted apart,
  # the same check as `mix app.migrations --check`
  test "every exported client migration is up to date" do
    problems = ClientMigrations.check()
    assert problems == [], Enum.map_join(problems, "\n", &ClientMigrations.describe/1)
  end

  test "exports only the synced tables, one file per migration that touches one" do
    export = ClientMigrations.export()

    assert Map.keys(export) == Enum.sort(Map.keys(export))
    assert Enum.any?(Map.keys(export), &String.ends_with?(&1, "_create_app_users.sql"))

    sql = Enum.join(Map.values(export))
    assert sql =~ ~s(CREATE TABLE "app_users")
    assert sql =~ ~s(CREATE TABLE "app_posts_communities")
    refute sql =~ ~s("users")
    refute sql =~ ~s("users_tokens")
    refute sql =~ ~s("app_purges")
    refute sql =~ "schema_migrations"
    refute sql =~ "BEGIN"
  end

  test "the exported schema, applied to an empty file, accepts a row pulled from the server unchanged",
       %{tmp_dir: tmp_dir} do
    owner = user_scope_fixture(user_fixture())
    community = joined_community_fixture(owner)
    post = post_fixture(owner, [community.id])
    {:ok, page} = Sync.pull(owner, %{})
    %{"rows" => rows, "next" => nil} = page |> Jason.encode!() |> Jason.decode!()

    {:ok, conn} = Sqlite3.open(Path.join(tmp_dir, "client.db"))
    :ok = Sqlite3.execute(conn, "PRAGMA foreign_keys = ON")

    for file <- Path.wildcard(Path.join(ClientMigrations.dir(), "*.sql")) |> Enum.sort(),
        statement <- statements(File.read!(file)) do
      :ok = Sqlite3.execute(conn, statement)
    end

    :ok = Sqlite3.execute(conn, "BEGIN")
    :ok = Sqlite3.execute(conn, "PRAGMA defer_foreign_keys = ON")

    for %{"table" => table, "row" => row} <- rows do
      columns = Map.keys(row)
      placeholders = Enum.map_join(columns, ", ", fn _ -> "?" end)
      insert = "INSERT INTO \"#{table}\" (#{Enum.join(columns, ", ")}) VALUES (#{placeholders})"
      {:ok, statement} = Sqlite3.prepare(conn, insert)
      :ok = Sqlite3.bind(statement, Enum.map(columns, &row[&1]))
      :done = Sqlite3.step(conn, statement)
    end

    :ok = Sqlite3.execute(conn, "COMMIT")

    {:ok, statement} = Sqlite3.prepare(conn, ~s(SELECT * FROM "app_posts" WHERE id = ?))
    :ok = Sqlite3.bind(statement, [post.id])
    {:ok, columns} = Sqlite3.columns(conn, statement)
    {:ok, [values]} = Sqlite3.fetch_all(conn, statement)

    [%{"row" => pulled}] =
      for %{"table" => "app_posts", "row" => %{"id" => id} = row} <- rows, id == post.id, do: %{"row" => row}

    assert Map.new(Enum.zip(columns, values)) == pulled
    assert pulled["updated_at"] =~ ~r/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$/

    :ok = Sqlite3.close(conn)
  end

  # As a client reads a file: comment lines dropped, statements split on
  # the `;` that ends each
  defp statements(sql) do
    sql
    |> String.split("\n")
    |> Enum.reject(&String.starts_with?(&1, "--"))
    |> Enum.join("\n")
    |> String.split(";\n", trim: true)
  end
end

defmodule CoreWeb.FilesController do
  use CoreWeb, :controller
  alias Core.App.Files

  def list(conn, params) do
    with {:ok, listed} <- Files.list(params["actor_id"], params["filter"] || %{}), do: json(conn, listed)
  end

  def get(conn, params) do
    with {:ok, file} <- Files.get(params["actor_id"], params["id"]), do: json(conn, file)
  end

  def create(conn, params) do
    with {:ok, file} <- Files.create(params["actor_id"], params["file"], params["uploaded"] == true),
         do: json(conn, file)
  end

  def update(conn, params) do
    with {:ok, file} <- Files.update(params["actor_id"], params["id"], params["name"], params["community_ids"] || []),
         do: json(conn, file)
  end

  def delete(conn, params) do
    with :ok <- Files.delete(params["actor_id"], params["id"]), do: json(conn, %{})
  end

  def readable(conn, params) do
    with {:ok, readable} <- Files.readable(params["actor_id"], params["id"]), do: json(conn, readable)
  end

  def storage_keys_list(conn, _params), do: json(conn, %{"keys" => Files.storage_keys_list()})

  def duplicates_list(conn, params), do: json(conn, Files.duplicates_list(params["actor_id"], params["sha256s"] || []))
end

defmodule CoreWeb.DeletionsController do
  use CoreWeb, :controller
  alias Core.Accounts.Deletions

  def code_create(conn, params) do
    with {:ok, sent} <- Deletions.code_create(params["user_id"], params["locale"] || "en"), do: json(conn, sent)
  end

  def create(conn, params) do
    with {:ok, deleted} <- Deletions.create(params["user_id"], params["code"], params["locale"] || "en"),
         do: json(conn, deleted)
  end
end

defmodule App.Web.SyncController do
  @moduledoc """
  The three sync endpoints of `docs/design/offline-sync.md`, over
  `App.Data.Sync`, with the API token as a bearer token:

      POST /sync/pull                    {"cursor": ..., "after": ..., "limit": n}
      POST /sync/push                    {"rows": [{"table": ..., "row": {...}}], "events": [...]}
      POST /sync/backfill/:community_id  {"after": ..., "limit": n}

  Like the websocket API, they refuse every request unless
  `API_SOCKET=true`, see `config/runtime.exs`. Try one with curl:

      curl -s -X POST http://localhost:4000/sync/pull \\
        -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{}'
  """

  use App.Web, :controller

  alias App.Data.Sync
  alias App.Web.ApiSocket
  alias App.Web.Scope

  plug :require_api_enabled
  plug :require_user

  def pull(conn, params) do
    case Sync.pull(conn.assigns.current_scope, params) do
      {:ok, page} -> json(conn, page)
      {:error, :bad_cursor} -> bad_request(conn, "cursor and after must hold a UTC datetime and an id")
    end
  end

  def push(conn, params) do
    rows = Map.get(params, "rows", [])
    events = Map.get(params, "events", [])

    if is_list(rows) and is_list(events) do
      json(conn, Sync.push(conn.assigns.current_scope, rows, events))
    else
      bad_request(conn, "rows and events must be lists")
    end
  end

  def backfill(conn, %{"community_id" => community_id} = params) do
    case Sync.backfill(conn.assigns.current_scope, community_id, params) do
      {:ok, page} ->
        json(conn, page)

      {:error, :not_a_member} ->
        conn
        |> put_status(:forbidden)
        |> json(%{"errors" => %{"detail" => "Forbidden"}})

      {:error, :bad_cursor} ->
        bad_request(conn, "after must hold a UTC datetime and an id")
    end
  end

  defp bad_request(conn, detail) do
    conn
    |> put_status(:bad_request)
    |> json(%{"errors" => %{"detail" => detail}})
  end

  defp require_user(conn, _opts) do
    case conn.assigns.current_scope do
      %Scope{user: %{}} ->
        conn

      _ ->
        conn
        |> put_status(:unauthorized)
        |> json(%{"errors" => %{"detail" => "Unauthorized"}})
        |> halt()
    end
  end

  defp require_api_enabled(conn, _opts) do
    if ApiSocket.enabled?() do
      conn
    else
      conn
      |> put_status(:not_found)
      |> json(%{"errors" => %{"detail" => "Not Found"}})
      |> halt()
    end
  end
end

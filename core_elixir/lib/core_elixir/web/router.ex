defmodule CoreElixir.Web.Router do
  @moduledoc """
  The private HTTP API Axum calls, see `docs/design/backend-protocol.md`:
  `POST /backend/<resource>/<action>` with a JSON body, behind the shared
  service token.
  """
  use Plug.Router

  alias CoreElixir.{Accounts, Sync}
  alias CoreElixir.App.{Communities, Files, FilesCommunities, Memberships, Users}

  plug :check_service_token
  plug Plug.Parsers, parsers: [:json], json_decoder: Jason
  plug :match
  plug :dispatch

  post "/backend/users/get_by_token", do: answer(conn, Users.get_by_token(conn.body_params["token"]))

  post "/backend/accounts/sign_in" do
    %{"email" => email} = body = conn.body_params
    answer(conn, Accounts.sign_in(email, Map.get(body, "sign_up_enabled", false), Map.get(body, "locale", "en")))
  end

  post "/backend/accounts/sign_in_confirm",
    do: answer(conn, Accounts.sign_in_confirm(conn.body_params["pending"], conn.body_params["code"]))

  post "/backend/accounts/sign_out" do
    Accounts.sign_out(conn.body_params["token"])
    answer(conn, {:ok, %{}})
  end

  post "/backend/sync/push" do
    body = conn.body_params
    answer(conn, Sync.push(body["actor_id"], body["events"] || [], body["uploaded"]))
  end

  post "/backend/sync/pull", do: answer(conn, Sync.pull(conn.body_params["actor_id"], conn.body_params))
  post "/backend/sync/backfill", do: answer(conn, Sync.backfill(conn.body_params["actor_id"], conn.body_params))

  post "/backend/communities/list", do: answer(conn, {:ok, Communities.list(conn.body_params["actor_id"])})

  post "/backend/memberships/create",
    do: answer(conn, Memberships.create(conn.body_params["actor_id"], conn.body_params["community_id"]))

  post "/backend/memberships/delete",
    do: answer(conn, Memberships.delete(conn.body_params["actor_id"], conn.body_params["community_id"]))

  post "/backend/files/list",
    do: answer(conn, Files.list(conn.body_params["actor_id"], conn.body_params["filter"] || %{}))

  post "/backend/files/get", do: answer(conn, Files.get(conn.body_params["actor_id"], conn.body_params["id"]))

  post "/backend/files/create" do
    body = conn.body_params
    answer(conn, Files.create(body["actor_id"], body["file"], body["uploaded"] == true))
  end

  post "/backend/files/update" do
    body = conn.body_params
    answer(conn, Files.update(body["actor_id"], body["id"], body["name"], body["community_ids"] || []))
  end

  post "/backend/files/delete" do
    answer(conn, with(:ok <- Files.delete(conn.body_params["actor_id"], conn.body_params["id"]), do: {:ok, %{}}))
  end

  post "/backend/files/readable", do: answer(conn, Files.readable(conn.body_params["actor_id"], conn.body_params["id"]))
  post "/backend/files/storage_keys_list", do: answer(conn, {:ok, %{"keys" => Files.storage_keys_list()}})

  post "/backend/files/duplicates_list",
    do: answer(conn, {:ok, Files.duplicates_list(conn.body_params["actor_id"], conn.body_params["sha256s"] || [])})

  post "/backend/files_communities/create" do
    body = conn.body_params
    answer(conn, FilesCommunities.create(body["actor_id"], body["file_id"], body["community_id"]))
  end

  post "/backend/files_communities/delete" do
    body = conn.body_params
    answer(conn, FilesCommunities.delete(body["actor_id"], body["file_id"], body["community_id"]))
  end

  post "/backend/:resource/:action",
    do: error(conn, 501, "not_implemented", "#{resource}/#{action} is not in the sample")

  match _, do: error(conn, 404, "not_found", "no such route")

  defp answer(conn, {:ok, body}), do: json(conn, 200, body)
  defp answer(conn, {:error, :unknown_token}), do: error(conn, 401, "unknown_token", "the token is not known")
  defp answer(conn, {:error, :account_deleted}), do: error(conn, 410, "account_deleted", "this account was deleted")
  defp answer(conn, {:error, :not_a_member}), do: error(conn, 403, "not_a_member", "not a member of the community")
  defp answer(conn, {:error, :not_found}), do: error(conn, 404, "not_found", "not found")
  defp answer(conn, {:error, :exists}), do: error(conn, 409, "exists", "it exists already")
  defp answer(conn, {:error, :sign_in_expired}), do: error(conn, 422, "sign_in_expired", "the sign-in expired")

  defp answer(conn, {:error, :wrong_code}),
    do: error(conn, 422, "wrong_code", "the code is wrong", %{"code" => ["is wrong"]})

  defp answer(conn, {:error, :invalid_continuation}),
    do: error(conn, 400, "invalid_continuation", "invalid sync continuation")

  defp answer(conn, {:error, {:invalid, fields}}), do: error(conn, 400, "invalid", "invalid", fields)
  defp answer(conn, {:error, {:refused, reason}}), do: error(conn, 400, "invalid", reason)

  defp error(conn, status, code, message, fields \\ nil) do
    body = %{"code" => code, "message" => message}
    body = if fields, do: Map.put(body, "fields", fields), else: body
    json(conn, status, %{"error" => body})
  end

  defp json(conn, status, body) do
    conn
    |> put_resp_content_type("application/json")
    |> send_resp(status, Jason.encode!(body))
  end

  defp check_service_token(conn, _opts) do
    expected = "Bearer " <> Application.fetch_env!(:core_elixir, :http)[:service_token]

    case get_req_header(conn, "authorization") do
      [^expected] -> conn
      _ -> conn |> error(401, "unauthorized", "a wrong service token") |> halt()
    end
  end
end

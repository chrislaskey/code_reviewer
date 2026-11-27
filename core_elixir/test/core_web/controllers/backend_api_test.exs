defmodule CoreWeb.BackendApiTest do
  use Core.BackendCase, async: false
  import Phoenix.ConnTest
  import Plug.Conn

  @endpoint CoreWeb.Endpoint

  defp call(path, body, token \\ "test-service-token") do
    build_conn()
    |> put_req_header("content-type", "application/json")
    |> put_req_header("authorization", "Bearer " <> token)
    |> post(path, Jason.encode!(body))
  end

  test "backend_refuses_a_wrong_service_token" do
    conn = call("/backend/users/get_by_token", %{"token" => "x"}, "wrong")
    assert %{"error" => %{"code" => "unauthorized"}} = json_response(conn, 401)
  end

  test "backend_signs_in_in_three_steps_resolves_the_token_and_pulls" do
    conn = call("/backend/accounts/sign_in", %{"email" => "alice@example.com", "sign_up_enabled" => true})
    %{"pending" => pending, "mail" => %{"kind" => "sign_in_code", "code" => code}} = json_response(conn, 200)

    conn = call("/backend/accounts/sign_in_with_code", %{"pending" => pending, "code" => "000000x"})
    assert %{"error" => %{"code" => "wrong_code"}} = json_response(conn, 422)

    conn = call("/backend/accounts/sign_in_with_code", %{"pending" => pending, "code" => code})
    %{"pending" => confirm, "new_account" => true} = json_response(conn, 200)

    conn = call("/backend/accounts/sign_in_confirm", %{"pending" => confirm})
    %{"token" => token, "user" => %{"id" => alice}} = json_response(conn, 200)

    conn = call("/backend/accounts/sign_in_confirm", %{"pending" => confirm})
    assert %{"error" => %{"code" => "sign_in_expired"}} = json_response(conn, 422)

    conn = call("/backend/users/get_by_token", %{"token" => token})
    assert %{"user" => %{"id" => ^alice}, "kind" => "account"} = json_response(conn, 200)

    conn = call("/backend/accounts/users/get", %{"user_id" => alice})
    assert %{"id" => ^alice, "email" => "alice@example.com", "trial_expires_at" => nil} = json_response(conn, 200)

    conn = call("/backend/sync/pull", %{"actor_id" => alice, "cursor" => nil, "limit" => 200})
    assert %{"rows" => [_, _, _], "cursor" => %{"global_sync_version" => _}} = json_response(conn, 200)
  end

  test "backend_answers_an_operation_outside_the_sample_not_implemented" do
    conn = call("/backend/posts/list", %{"actor_id" => Core.Helpers.Ids.new()})
    assert %{"error" => %{"code" => "not_implemented"}} = json_response(conn, 501)
  end

  test "backend_answers_a_file_it_cannot_find_not_found" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    conn = call("/backend/files/get", %{"actor_id" => alice, "id" => Core.Helpers.Ids.new()})
    assert %{"error" => %{"code" => "not_found"}} = json_response(conn, 404)
  end
end

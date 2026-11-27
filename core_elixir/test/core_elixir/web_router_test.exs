defmodule CoreElixir.Web.RouterTest do
  use CoreElixir.BackendCase, async: false
  import Plug.Test
  import Plug.Conn

  alias CoreElixir.Web.Router

  defp call(path, body, token \\ "test-service-token") do
    conn(:post, path, Jason.encode!(body))
    |> put_req_header("content-type", "application/json")
    |> put_req_header("authorization", "Bearer " <> token)
    |> Router.call(Router.init([]))
  end

  test "router_refuses_a_wrong_service_token" do
    conn = call("/backend/users/get_by_token", %{"token" => "x"}, "wrong")
    assert conn.status == 401
    assert %{"error" => %{"code" => "unauthorized"}} = Jason.decode!(conn.resp_body)
  end

  test "router_signs_in_resolves_the_token_and_pulls" do
    conn = call("/backend/accounts/sign_in", %{"email" => "alice@example.com", "sign_up_enabled" => true})
    %{"pending" => pending, "mail" => %{"kind" => "sign_in_code", "code" => code}} = Jason.decode!(conn.resp_body)

    conn = call("/backend/accounts/sign_in_confirm", %{"pending" => pending, "code" => "000000x"})
    assert conn.status == 422

    conn = call("/backend/accounts/sign_in_confirm", %{"pending" => pending, "code" => code})
    %{"token" => token, "user" => %{"id" => alice}} = Jason.decode!(conn.resp_body)

    conn = call("/backend/users/get_by_token", %{"token" => token})
    assert %{"user" => %{"id" => ^alice}, "kind" => "account"} = Jason.decode!(conn.resp_body)

    conn = call("/backend/sync/pull", %{"actor_id" => alice, "cursor" => nil, "limit" => 200})
    assert %{"rows" => [_, _, _], "cursor" => %{"global_sync_version" => _}} = Jason.decode!(conn.resp_body)
  end

  test "router_answers_an_operation_outside_the_sample_not_implemented" do
    conn = call("/backend/posts/list", %{"actor_id" => CoreElixir.Helpers.Ids.new()})
    assert conn.status == 501
    assert %{"error" => %{"code" => "not_implemented"}} = Jason.decode!(conn.resp_body)
  end
end

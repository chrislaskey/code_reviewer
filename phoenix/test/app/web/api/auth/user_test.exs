defmodule App.Web.Api.Auth.UserTest do
  use App.Web.ChannelCase

  import App.Data.UsersFixtures

  alias App.Data.Accounts
  alias App.Web.ApiChannel
  alias App.Web.ApiSocket

  setup do
    user = user_fixture()
    token = Accounts.generate_user_api_token(user)
    {:ok, socket} = connect(ApiSocket, %{}, connect_info: %{auth_token: token})
    {:ok, _reply, socket} = subscribe_and_join(socket, ApiChannel, "api")
    %{user: user, socket: socket}
  end

  describe "auth.user.get" do
    test "replies with the connected user", %{user: user, socket: socket} do
      ref = push(socket, "auth.user.get", %{})

      assert_reply ref, :ok, %{id: user_id, email: email}
      assert user_id == user.id
      assert email == user.email
    end
  end
end

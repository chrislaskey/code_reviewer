defmodule CoreWeb.Router do
  use CoreWeb, :router

  alias CoreWeb.{
    AccountsController,
    CommunitiesController,
    FilesCommunitiesController,
    FilesController,
    MembershipsController,
    NotImplementedController,
    SyncController,
    UsersController
  }

  pipeline :backend do
    plug :accepts, ["json"]
    plug CoreWeb.Plugs.ServiceToken
  end

  scope "/backend" do
    pipe_through :backend

    post "/users/get_by_token", UsersController, :get_by_token

    post "/accounts/users/get", AccountsController, :users_get
    post "/accounts/sign_in", AccountsController, :sign_in
    post "/accounts/sign_in_with_code", AccountsController, :sign_in_with_code
    post "/accounts/resend_code_to", AccountsController, :resend_code_to
    post "/accounts/resend_code", AccountsController, :resend_code
    post "/accounts/sign_in_confirm", AccountsController, :sign_in_confirm
    post "/accounts/sign_in_confirm_for_device", AccountsController, :sign_in_confirm_for_device
    post "/accounts/sign_in_with_device_code", AccountsController, :sign_in_with_device_code
    post "/accounts/sign_out", AccountsController, :sign_out

    post "/sync/push", SyncController, :push
    post "/sync/pull", SyncController, :pull
    post "/sync/backfill", SyncController, :backfill

    post "/communities/list", CommunitiesController, :list
    post "/memberships/create", MembershipsController, :create
    post "/memberships/delete", MembershipsController, :delete

    post "/files/list", FilesController, :list
    post "/files/get", FilesController, :get
    post "/files/create", FilesController, :create
    post "/files/update", FilesController, :update
    post "/files/delete", FilesController, :delete
    post "/files/readable", FilesController, :readable
    post "/files/storage_keys_list", FilesController, :storage_keys_list
    post "/files/duplicates_list", FilesController, :duplicates_list

    post "/files_communities/create", FilesCommunitiesController, :create
    post "/files_communities/delete", FilesCommunitiesController, :delete

    post "/:resource/:action", NotImplementedController, :show
  end
end

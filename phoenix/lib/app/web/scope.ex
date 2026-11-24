defmodule App.Web.Scope do
  @moduledoc """
  Defines the scope of the caller to be used throughout the app.

  The `App.Web.Scope` allows public interfaces to receive
  information about the caller, such as if the call is initiated from an
  end-user, and if so, which user. Additionally, such a scope can carry fields
  such as "super user" or other privileges for use in authorization checks,
  or to ensure specific code paths can only be accessed for a given scope.

  It is useful for logging as well as for scoping pubsub subscriptions and
  broadcasts when a caller subscribes to an interface or performs a particular
  action.

  ## Community

  `community` is the `App.Data.Community` of a request inside one, set by
  `App.Web.CommunityAuth` once it has checked the user is a member.
  """

  alias App.Data.Community
  alias App.Data.User

  defstruct user: nil, community: nil

  @doc """
  Creates a scope for the given user.

  Returns nil if no user is given.
  """
  def for_user(%User{} = user) do
    %__MODULE__{user: user}
  end

  def for_user(nil), do: nil

  @doc """
  The scope with the community the user is a member of.
  """
  def put_community(%__MODULE__{} = scope, %Community{} = community),
    do: %{scope | community: community}
end

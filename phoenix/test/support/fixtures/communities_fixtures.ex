defmodule App.Data.CommunitiesFixtures do
  @moduledoc """
  Test helpers for communities and memberships, via
  `App.Data.Communities` and `App.Data.Memberships`.
  """

  alias App.Data.Communities
  alias App.Data.Memberships

  def unique_community_name, do: "community#{System.unique_integer()}"

  @doc """
  A community the scope's user has not joined.
  """
  def community_fixture(attrs \\ %{}) do
    {:ok, community} =
      Communities.create(Enum.into(attrs, %{name: unique_community_name()}))

    community
  end

  @doc """
  A community the scope's user is a member of.
  """
  def joined_community_fixture(scope, attrs \\ %{}) do
    community = community_fixture(attrs)
    {:ok, _membership} = Memberships.join(scope, community.id)
    community
  end
end

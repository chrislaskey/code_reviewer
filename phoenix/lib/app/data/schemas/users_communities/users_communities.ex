defmodule App.Data.UsersCommunities do
  @moduledoc """
  The data context over `App.Data.UserCommunity`. A soft-deleted
  membership is left out of every read here, so a user who
  left is not a member anywhere. The join, leave and rejoin flows are in
  `App.Data.Memberships`.

  The `visible_to: scope` filter is what syncs to a user: their own
  memberships, live or not, and the memberships of the communities they
  are a live member of.
  """

  use App.Data.Context, schema: App.Data.UserCommunity

  import Ecto.Query, only: [from: 2]

  alias App.Data.UserCommunity
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{user: user}) do
    my_communities =
      from(uc in UserCommunity,
        where: uc.user_id == ^user.id and is_nil(uc.deleted_at),
        select: uc.community_id
      )

    Ecto.Query.where(
      query,
      [uc],
      uc.user_id == ^user.id or uc.community_id in subquery(my_communities)
    )
  end

  def filter(query, key, value), do: super(query, key, value)
end

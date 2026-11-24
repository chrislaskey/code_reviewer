defmodule App.Data.Communities do
  @moduledoc """
  The data context over `App.Data.Community`. Any user may create one.
  Joining and leaving are in `App.Data.Memberships`.

  The `visible_to: scope` filter is what syncs to a user: the communities
  they are a live member of.
  """

  use App.Data.Context, schema: App.Data.Community, sorts: [:name]

  import Ecto.Query, only: [from: 2]

  alias App.Data.UserCommunity
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{user: user}) do
    my_communities =
      from(uc in UserCommunity,
        where: uc.user_id == ^user.id and is_nil(uc.deleted_at),
        select: uc.community_id
      )

    Ecto.Query.where(query, [c], c.id in subquery(my_communities))
  end

  def filter(query, key, value), do: super(query, key, value)
end

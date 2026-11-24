defmodule App.Data.UserProfiles do
  @moduledoc """
  The data context over `App.Data.UserProfile`. The `visible_to: scope`
  filter is what syncs to a user: their own profile.
  """

  use App.Data.Context, schema: App.Data.UserProfile

  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{user: user}) do
    Ecto.Query.where(query, [profile], profile.user_id == ^user.id)
  end

  def filter(query, key, value), do: super(query, key, value)
end

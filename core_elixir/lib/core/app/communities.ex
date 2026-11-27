defmodule Core.App.Communities do
  @moduledoc "Communities: the ones a user sees, and the personal one a sign-up starts with."
  alias Core.Data.{AppCommunities, AppUsersCommunities}
  alias Core.Helpers.{Ids, Time}

  @personal_name "Personal"

  @doc "Every community the actor sees, by name, with their live membership or nil."
  def list(actor_id) do
    memberships = actor_id |> AppUsersCommunities.list_live_of_user() |> Map.new(&{&1.community_id, &1})

    for community <- AppCommunities.list_live(actor_id) do
      membership = memberships[community.id]

      %{
        "community" => AppCommunities.to_wire(community),
        "membership" => membership && AppUsersCommunities.to_wire(membership)
      }
    end
  end

  @doc "Creates the user's personal community and their membership of it, published, in the caller's transaction."
  def create_personal_in(user_id) do
    now = Time.now()

    community =
      AppCommunities.create(%AppCommunities{
        id: Ids.new(),
        name: @personal_name,
        inserted_at: now,
        updated_at: now,
        version: 1,
        last_synced_at: now
      })

    AppUsersCommunities.create(%AppUsersCommunities{
      id: Ids.new(),
      user_id: user_id,
      community_id: community.id,
      inserted_at: now,
      updated_at: now,
      version: 1,
      last_synced_at: now
    })

    community
  end
end

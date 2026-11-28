defmodule Core.App.Communities do
  @moduledoc "Communities: the ones a user sees, and the personal one a sign-up starts with."
  alias Core.App.Memberships
  alias Core.Data.{AppCommunities, AppUsersCommunities}
  alias Core.Helpers.{Ids, Time}
  alias Core.Sync.{Answers, Events}

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

  @doc """
  Creates a community with the name and the user's membership of it,
  each published and recorded as the user's event, as
  `communities::create_in` does. In the caller's transaction.
  """
  def create_in(user_id, name) do
    now = Time.now()

    community =
      AppCommunities.create(%AppCommunities{id: Ids.new(), name: name, inserted_at: now, updated_at: now, version: 1})

    event = Events.create(AppCommunities.table(), community.id, %{"name" => name})
    Events.record(user_id, event, Answers.applied(event["id"], community.version, %{}, nil))
    Memberships.create_in(user_id, community.id)
    community
  end

  @doc "The name of the community a sign-up starts with."
  def personal_name, do: @personal_name
end

defmodule App.Data.Profiles do
  @moduledoc """
  Editing a user's own profile, from the web and from devices alike, as
  events merged field by field, see "Events" in
  `docs/design/offline-sync.md`. Profiles keep no server event log.
  """

  alias App.Data.Sync.Merge
  alias App.Data.UserProfile
  alias App.Data.UserProfiles
  alias App.Web.Scope

  @event_fields ["display_name", "bio"]

  @doc """
  The scope's user's profile.
  """
  def get(%Scope{user: user}), do: UserProfiles.get_by(user_id: user.id)

  @doc """
  Saves the fields of `params` that differ from `loaded`, the profile as
  the form loaded it. Returns what `apply_event/2` does.
  """
  def save(%Scope{} = scope, %UserProfile{} = loaded, params) do
    loaded_values = Merge.values(loaded, @event_fields)

    changes =
      for field <- @event_fields,
          Map.has_key?(params, field),
          value = blank_to_nil(params[field]),
          value != loaded_values[field],
          into: %{},
          do: {field, value}

    apply_event(scope, %{
      id: Ecto.UUID.generate(version: 7),
      row_id: loaded.id,
      type: :update,
      base_version: loaded.version,
      base: Map.take(loaded_values, Map.keys(changes)),
      changes: changes
    })
  end

  defp blank_to_nil(value) when value in [nil, ""], do: nil
  defp blank_to_nil(value), do: value

  @doc """
  Applies an event to the scope's user's own profile. Returns
  `{:ok, %{version: version, conflicts: conflicts}, profile}`,
  `{:error, changeset}`, or `{:error, reason, profile_or_nil}`.
  """
  def apply_event(%Scope{user: %{id: user_id}}, %{type: :update, changes: changes} = event) do
    with [] <- Map.keys(changes) -- @event_fields,
         %UserProfile{user_id: ^user_id} = profile <- UserProfiles.get(event.row_id) do
      values = Merge.values(profile, @event_fields)
      {applied, conflicts} = Merge.merge(values, profile.version, event.base_version, event.base, changes)

      with {:ok, profile} <- update(profile, applied) do
        {:ok, %{version: profile.version, conflicts: conflicts}, profile}
      end
    else
      nil -> {:error, "unknown_profile", nil}
      %UserProfile{} = profile -> {:error, "not_owner", profile}
      fields -> {:error, "not_editable: " <> Enum.join(fields, ", "), nil}
    end
  end

  def apply_event(_scope, _event), do: {:error, "not_creatable", nil}

  defp update(profile, applied) when applied == %{}, do: {:ok, profile}
  defp update(profile, applied), do: UserProfiles.update(profile, applied)
end

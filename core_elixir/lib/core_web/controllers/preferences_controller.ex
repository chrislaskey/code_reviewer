defmodule CoreWeb.PreferencesController do
  use CoreWeb, :controller
  alias Core.Accounts.Preferences

  def get(conn, params), do: json(conn, Preferences.get(params["user_id"]))

  def update(conn, params) do
    with {:ok, preferences} <- Preferences.update(params["user_id"], params["key"], params["value"]),
         do: json(conn, preferences)
  end
end

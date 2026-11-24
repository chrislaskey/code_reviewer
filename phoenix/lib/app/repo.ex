defmodule App.Repo do
  @moduledoc """
  The one database of a deployment: users and their tokens, and the app's
  own tables, prefixed `app_`. See `docs/design/multi-tenancy.md`.
  """

  use Ecto.Repo,
    otp_app: :app,
    adapter: Ecto.Adapters.SQLite3
end

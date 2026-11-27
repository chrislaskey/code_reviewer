defmodule Core.Repo do
  @moduledoc "The backend's database, which it alone reads and writes."
  use Ecto.Repo, otp_app: :core, adapter: Ecto.Adapters.Postgres
end

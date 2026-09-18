defmodule CodeReviewer.Repo do
  use Ecto.Repo,
    otp_app: :code_reviewer,
    adapter: Ecto.Adapters.SQLite3
end

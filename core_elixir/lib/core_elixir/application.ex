defmodule CoreElixir.Application do
  @moduledoc false
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      CoreElixir.Repo,
      {Oban, Application.fetch_env!(:core_elixir, Oban)},
      CoreElixir.CommandedApplication,
      CoreElixir.App.Files.Projector,
      CoreElixirWeb.Endpoint
    ]

    Supervisor.start_link(children, strategy: :one_for_one, name: CoreElixir.Supervisor)
  end
end

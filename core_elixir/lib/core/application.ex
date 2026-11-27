defmodule Core.Application do
  @moduledoc false
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      Core.Repo,
      {Oban, Application.fetch_env!(:core, Oban)},
      Core.CommandedApplication,
      Core.App.Files.Projector,
      CoreWeb.Endpoint
    ]

    Supervisor.start_link(children, strategy: :one_for_one, name: Core.Supervisor)
  end
end

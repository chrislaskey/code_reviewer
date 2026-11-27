defmodule CoreElixir.Application do
  @moduledoc false
  use Application

  @impl true
  def start(_type, _args) do
    http = Application.fetch_env!(:core_elixir, :http)

    children =
      [
        CoreElixir.Repo,
        {Oban, Application.fetch_env!(:core_elixir, Oban)},
        CoreElixir.CommandedApplication,
        CoreElixir.App.Files.Projector
      ] ++
        if Keyword.get(http, :start, true),
          do: [{Bandit, plug: CoreElixir.Web.Router, port: Keyword.fetch!(http, :port)}],
          else: []

    Supervisor.start_link(children, strategy: :one_for_one, name: CoreElixir.Supervisor)
  end
end

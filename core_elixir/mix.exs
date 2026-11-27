defmodule CoreElixir.MixProject do
  use Mix.Project

  def project do
    [
      app: :core_elixir,
      version: "0.1.0",
      elixir: "~> 1.18",
      elixirc_paths: elixirc_paths(Mix.env()),
      start_permanent: Mix.env() == :prod,
      aliases: aliases(),
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger, :crypto],
      mod: {CoreElixir.Application, []}
    ]
  end

  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_), do: ["lib"]

  defp deps do
    [
      {:bandit, "~> 1.15"},
      {:commanded, "~> 1.4"},
      {:commanded_ecto_projections, "~> 1.4"},
      {:commanded_eventstore_adapter, "~> 1.4"},
      {:ecto_sql, "~> 3.14"},
      {:eventstore, "~> 1.4"},
      {:jason, "~> 1.4"},
      {:oban, "~> 2.24"},
      {:plug, "~> 1.20"},
      {:postgrex, "~> 0.23.0"}
    ]
  end

  defp aliases do
    [
      setup: ["deps.get", "database.create"],
      "database.create": ["ecto.create", "event_store.create", "event_store.init", "ecto.migrate"],
      "database.reset": ["ecto.drop", "database.create"],
      test: [
        "ecto.create --quiet",
        "event_store.create --quiet",
        "event_store.init --quiet",
        "ecto.migrate --quiet",
        "test"
      ]
    ]
  end
end

defmodule Mix.Tasks.Core.Migrate do
  @shortdoc "Runs the Rust core's PostgreSQL migrations, then the backend's own"
  @moduledoc "Runs every migration of `Core.Migrations` not yet run."
  use Mix.Task

  @impl Mix.Task
  def run(_args) do
    Mix.Task.run("app.config")
    Core.Migrations.migrate()
  end
end

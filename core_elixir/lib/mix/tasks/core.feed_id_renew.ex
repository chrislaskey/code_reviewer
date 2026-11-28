defmodule Mix.Tasks.Core.FeedIdRenew do
  @shortdoc "Gives the sync feed a new id: run it once after restoring a backup"
  @moduledoc """
  Gives the sync feed a new id, so every device resyncs at its next
  pull, see "Publishing a change" in `docs/design/backend-protocol.md`.
  Run it once after restoring a backup, before the backend serves again.
  In a release: `Core.Migrations.feed_id_renew/0`.
  """
  use Mix.Task

  @impl Mix.Task
  def run(_args) do
    Mix.Task.run("app.config")
    Core.Migrations.feed_id_renew()
    Mix.shell().info("The feed has a new id; every device resyncs at its next pull.")
  end
end

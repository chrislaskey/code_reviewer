defmodule Mix.Tasks.App.Purge do
  @shortdoc "Hard-deletes rows soft-deleted more than 90 days ago"

  @moduledoc """
  Runs the purge job, see `App.Data.Purge`. Run it on a schedule, daily is
  plenty.

      $ mix app.purge
  """

  use Mix.Task

  @impl true
  def run(_args) do
    Mix.Task.run("app.start")
    result = App.Data.Purge.run()
    Mix.shell().info("purged up to #{DateTime.to_iso8601(result.horizon)}")
    Mix.shell().info(inspect(result.counts))
  end
end

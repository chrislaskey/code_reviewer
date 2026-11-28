defmodule Core.Jobs.TrialUsersDelete do
  @moduledoc """
  The hourly removal of expired trial users, as `TrialUsersDelete` in
  the Rust core, see `Core.Accounts.TrialUsers.delete_expired/1`.
  """
  use Oban.Worker, queue: :maintenance, max_attempts: 3

  alias Core.Accounts.TrialUsers
  alias Core.Helpers.Time

  @impl Oban.Worker
  def perform(_job) do
    {:ok, _} = TrialUsers.delete_expired(Time.now())
    :ok
  end
end

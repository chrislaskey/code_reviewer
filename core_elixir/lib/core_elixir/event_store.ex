defmodule CoreElixir.EventStore do
  @moduledoc "Commanded's event store, in the `event_store` schema of the same database."
  use EventStore, otp_app: :core_elixir
end

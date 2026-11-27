defmodule CoreElixir.CommandedApplication do
  @moduledoc "The Commanded application: the router and the event store it appends to."
  use Commanded.Application, otp_app: :core_elixir

  router(CoreElixir.Router)
end

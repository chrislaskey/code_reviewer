defmodule App.Data.Events do
  @moduledoc """
  The data context over `App.Data.Event`, the shared event log, in stream
  order. Recording, and which resources keep a log, are in
  `App.Data.EventLog`.
  """

  use App.Data.Context, schema: App.Data.Event, sorts: [:sequence]
end

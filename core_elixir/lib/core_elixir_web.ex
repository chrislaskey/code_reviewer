defmodule CoreElixirWeb do
  @moduledoc """
  The private HTTP API Axum calls, see `docs/design/backend-protocol.md`:
  `POST /backend/<resource>/<action>` with a JSON body, behind the shared
  service token. JSON only: no pages, no assets, no sessions.
  """

  def router do
    quote do
      use Phoenix.Router, helpers: false
      import Plug.Conn
      import Phoenix.Controller
    end
  end

  def controller do
    quote do
      use Phoenix.Controller, formats: [:json]
      import Plug.Conn
      action_fallback CoreElixirWeb.FallbackController
    end
  end

  defmacro __using__(which) when is_atom(which), do: apply(__MODULE__, which, [])
end

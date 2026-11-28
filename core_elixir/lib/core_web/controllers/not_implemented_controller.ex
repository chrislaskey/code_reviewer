defmodule CoreWeb.NotImplementedController do
  @moduledoc "Answers an operation the protocol names that the sample does not implement."
  use CoreWeb, :controller

  def show(conn, %{"resource" => resource, "action" => action}) do
    CoreWeb.FallbackController.error(conn, 501, "not_implemented", "#{resource}/#{action} is not in the sample")
  end
end

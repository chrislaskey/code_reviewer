defmodule CoreWeb.ErrorJSON do
  @moduledoc "The protocol's error answer for what Phoenix itself refuses, such as malformed JSON."
  def render(template, _assigns) do
    %{"error" => %{"code" => "invalid", "message" => Phoenix.Controller.status_message_from_template(template)}}
  end
end

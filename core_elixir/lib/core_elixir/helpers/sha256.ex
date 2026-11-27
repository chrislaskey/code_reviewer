defmodule CoreElixir.Helpers.Sha256 do
  @moduledoc "SHA-256, as hex."

  @doc "The SHA-256 of the text, as lower-case hex."
  def hex(text), do: :crypto.hash(:sha256, text) |> Base.encode16(case: :lower)

  @doc "Whether the text is a SHA-256 as hex."
  def valid?(text), do: is_binary(text) and text =~ ~r/\A[0-9a-f]{64}\z/
end

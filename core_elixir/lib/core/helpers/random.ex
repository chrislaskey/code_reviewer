defmodule Core.Helpers.Random do
  @moduledoc "Random tokens and codes, from the system's secure generator."

  @doc "`bytes` random bytes, as hex."
  def hex(bytes), do: :crypto.strong_rand_bytes(bytes) |> Base.encode16(case: :lower)

  @doc "`length` random letters and digits, as a made-up name."
  def letters_and_digits(length) do
    alphabet = ~c"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    for _ <- 1..length, into: "", do: <<Enum.random(alphabet)>>
  end

  @doc "A code of `count` random digits."
  def digits(count) do
    for _ <- 1..count, into: "", do: Integer.to_string(:rand.uniform(10) - 1)
  end
end

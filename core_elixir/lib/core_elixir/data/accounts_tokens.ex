defmodule CoreElixir.Data.AccountsTokens do
  @moduledoc """
  Token records: the `accounts_tokens` schema and its reads and writes.
  A token is kept as its SHA-256 hash, so reading the table gives
  nothing that works. Never synced.
  """
  use Ecto.Schema
  import Ecto.Query
  alias CoreElixir.Helpers.{Ids, Sha256, Time}
  alias CoreElixir.Repo

  @context_api "api"
  @context_sign_in_code "sign_in_code"
  @sign_in_code_validity_minutes 10

  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema "accounts_tokens" do
    field :user_id, Ecto.UUID
    field :context, :string
    field :token_hash, :string
    field :code_hash, :string
    field :attempts, :integer, default: 0
    field :inserted_at, :utc_datetime_usec
  end

  def context_api, do: @context_api
  def context_sign_in_code, do: @context_sign_in_code

  @doc "Inserts a token of the context for the user, by the token's hash, with the code's hash for a sign-in."
  def create(user_id, context, token, code \\ nil) do
    Repo.insert!(%__MODULE__{
      id: Ids.new(),
      user_id: user_id,
      context: context,
      token_hash: Sha256.hex(token),
      code_hash: code && Sha256.hex(code),
      inserted_at: Time.now()
    })
  end

  @doc "Gets the token of the context, by the token itself, or nil. A sign-in code expires after ten minutes."
  def get_valid(token, context) do
    row = Repo.one(from t in __MODULE__, where: t.token_hash == ^Sha256.hex(token) and t.context == ^context)

    cond do
      is_nil(row) -> nil
      context == @context_sign_in_code and expired?(row) -> nil
      true -> row
    end
  end

  @doc "Counts a wrong code against the sign-in, and answers the attempts so far."
  def update_attempts(row) do
    {1, [%{attempts: attempts}]} =
      Repo.update_all(
        from(t in __MODULE__, where: t.id == ^row.id, select: %{attempts: t.attempts}),
        inc: [attempts: 1]
      )

    attempts
  end

  @doc "Deletes the user's tokens of the context."
  def delete_of_user(user_id, context),
    do: Repo.delete_all(from t in __MODULE__, where: t.user_id == ^user_id and t.context == ^context)

  @doc "Deletes a token, by the token itself."
  def delete_by_token(token),
    do: Repo.delete_all(from t in __MODULE__, where: t.token_hash == ^Sha256.hex(token))

  @doc "Deletes a row."
  def delete(row), do: Repo.delete!(row)

  defp expired?(row) do
    DateTime.diff(Time.now(), row.inserted_at, :minute) >= @sign_in_code_validity_minutes
  end
end

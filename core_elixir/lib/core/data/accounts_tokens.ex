defmodule Core.Data.AccountsTokens do
  @moduledoc """
  Token records: the `accounts_tokens` schema and its reads and writes.
  A token is kept as its SHA-256 hash, so reading the table gives
  nothing that works. Never synced.
  """
  use Ecto.Schema
  import Ecto.Query
  alias Core.Helpers.{Ids, Sha256, Time}
  alias Core.Repo

  @context_api "api"
  @context_sign_in_code "sign_in_code"
  @context_sign_in_confirm "sign_in_confirm"
  @context_device_code "device_code"
  @sign_in_code_validity_minutes 10
  @device_code_validity_minutes 5

  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema "accounts_tokens" do
    field :user_id, Ecto.UUID
    field :context, :string
    field :token_hash, :string
    field :code_hash, :string
    field :attempts, :integer, default: 0
    field :sent_to, :string
    field :inserted_at, :utc_datetime_usec
  end

  def context_api, do: @context_api
  def context_sign_in_code, do: @context_sign_in_code
  def context_sign_in_confirm, do: @context_sign_in_confirm
  def context_device_code, do: @context_device_code

  @doc """
  Inserts a token of the context for the user, by the token's hash, with
  the code's hash for a sign-in, and the address the code went to when
  it is not yet the account's, as for a trial user signing up.
  """
  def create(user_id, context, token, code \\ nil, sent_to \\ nil) do
    Repo.insert!(%__MODULE__{
      id: Ids.new(),
      user_id: user_id,
      context: context,
      token_hash: Sha256.hex(token),
      code_hash: code && Sha256.hex(code),
      sent_to: sent_to,
      inserted_at: Time.now()
    })
  end

  @doc """
  Gets the token of the context, by the token itself, or nil. A sign-in's
  steps expire after ten minutes, a device code after five.
  """
  def get_valid(token, context) do
    row = Repo.one(from t in __MODULE__, where: t.token_hash == ^Sha256.hex(token) and t.context == ^context)

    cond do
      is_nil(row) -> nil
      expired?(row) -> nil
      true -> row
    end
  end

  @doc "Gives the sign-in a new code, and ten minutes again; answers the code."
  def update_code(row, code) do
    Repo.update_all(from(t in __MODULE__, where: t.id == ^row.id),
      set: [code_hash: Sha256.hex(code), attempts: 0, inserted_at: Time.now()]
    )

    code
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

  defp expired?(%{context: context} = row) when context in [@context_sign_in_code, @context_sign_in_confirm],
    do: DateTime.diff(Time.now(), row.inserted_at, :minute) >= @sign_in_code_validity_minutes

  defp expired?(%{context: @context_device_code} = row),
    do: DateTime.diff(Time.now(), row.inserted_at, :minute) >= @device_code_validity_minutes

  defp expired?(_row), do: false
end

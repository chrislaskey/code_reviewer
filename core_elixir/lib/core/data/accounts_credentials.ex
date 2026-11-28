defmodule Core.Data.AccountsCredentials do
  @moduledoc """
  Password records: the `accounts_credentials` schema and its reads and
  writes, one row per account that has a password, holding its hash.
  Rust hashes and checks passwords; the backend only keeps the hash.
  Never synced.
  """
  use Ecto.Schema
  alias Core.Repo

  @primary_key {:user_id, Ecto.UUID, autogenerate: false}
  schema "accounts_credentials" do
    field :password_hash, :string
    field :inserted_at, :utc_datetime_usec
    field :updated_at, :utc_datetime_usec
  end

  @doc "The user's row, or nil when they have no password."
  def get(user_id), do: Repo.get(__MODULE__, user_id)

  @doc "Keeps the hash as the user's password, in place of any they had."
  def upsert(credential) do
    Repo.insert!(credential,
      on_conflict: [set: [password_hash: credential.password_hash, updated_at: credential.updated_at]],
      conflict_target: :user_id
    )
  end
end

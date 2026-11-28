defmodule Core.Data.AccountsUsers do
  @moduledoc "Account records: the `accounts_users` schema and its reads and writes. Never synced."
  use Ecto.Schema
  import Ecto.Query
  alias Core.Helpers.Time
  alias Core.Repo

  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema "accounts_users" do
    field :email, :string
    field :confirmed_at, :utc_datetime_usec
    field :trial_expires_at, :utc_datetime_usec
    field :terms_version, :string
    field :terms_accepted_at, :utc_datetime_usec
    field :inserted_at, :utc_datetime_usec
    field :updated_at, :utc_datetime_usec
    field :deleted_at, :utc_datetime_usec
  end

  @doc "An address as it is kept: trimmed and in lower case; nil when it is not one."
  def checked_email(email) when is_binary(email) do
    email = email |> String.trim() |> String.downcase()
    if email =~ ~r/\A[^@\s]+@[^@\s]+\.[^@\s]+\z/, do: email, else: nil
  end

  def checked_email(_), do: nil

  @doc "The account as Rust's `AccountUser` is written."
  def to_wire(account) do
    %{
      "id" => account.id,
      "email" => account.email,
      "confirmed_at" => Time.to_wire(account.confirmed_at),
      "trial_expires_at" => Time.to_wire(account.trial_expires_at),
      "terms_version" => account.terms_version,
      "terms_accepted_at" => Time.to_wire(account.terms_accepted_at),
      "inserted_at" => Time.to_wire(account.inserted_at),
      "updated_at" => Time.to_wire(account.updated_at),
      "deleted_at" => Time.to_wire(account.deleted_at)
    }
  end

  @doc "Gets a row by id, deleted or not."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Gets the live account with the address, or nil."
  def get_by_email(email),
    do: Repo.one(from a in __MODULE__, where: a.email == ^email and is_nil(a.deleted_at))

  @doc "Inserts a new row."
  def create(account), do: Repo.insert!(account)

  @doc "Deletes an unconfirmed sign-up outright, its user's row first: someone else proved its address."
  def delete_unconfirmed(account) do
    Repo.delete_all(from u in Core.Data.AppUsers, where: u.id == ^account.id)
    Repo.delete!(account)
  end

  @doc "Writes the row over its stored one."
  def update(account), do: Repo.insert!(account, on_conflict: :replace_all, conflict_target: :id)
end

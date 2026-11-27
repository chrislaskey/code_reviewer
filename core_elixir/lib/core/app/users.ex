defmodule Core.App.Users do
  @moduledoc "Users: who a token stands for, and the user a sign-up makes."
  alias Core.Data.{AccountsTokens, AccountsUsers, AppUsers}
  alias Core.Helpers.Time

  @doc "The user a token stands for, with their address and kind, as `users::get_by_token` answers it."
  def get_by_token(token) when is_binary(token) do
    with %AccountsTokens{} = row <-
           AccountsTokens.get_valid(token, AccountsTokens.context_api()) || {:error, :unknown_token},
         %AccountsUsers{deleted_at: nil} = account <- AccountsUsers.get(row.user_id) || {:error, :unknown_token},
         %AppUsers{deleted_at: nil} = user <- AppUsers.get(row.user_id) || {:error, :unknown_token} do
      kind = if account.trial_expires_at, do: "trial", else: "account"
      {:ok, %{"user" => AppUsers.to_wire(user), "email" => account.email, "kind" => kind}}
    else
      %AccountsUsers{} -> {:error, :account_deleted}
      %AppUsers{} -> {:error, :account_deleted}
      error -> error
    end
  end

  def get_by_token(_token), do: {:error, :unknown_token}

  @doc "Creates a user's row, named from their address, and publishes it. In the caller's transaction."
  def create_in(id, email) do
    now = Time.now()
    name = email |> String.split("@") |> hd()

    AppUsers.create(%AppUsers{
      id: id,
      name: name,
      inserted_at: now,
      updated_at: now,
      version: 1,
      last_synced_at: now
    })
  end
end

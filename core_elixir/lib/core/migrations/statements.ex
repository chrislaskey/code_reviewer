defmodule Core.Migrations.Statements do
  @moduledoc """
  A SQL file as the statements PostgreSQL runs one at a time. The Rust
  migrations have no functions or `DO` blocks, so a `;` always ends a
  statement once comments are dropped.
  """

  @doc "The statements of a migration file: comments dropped, split at each `;`."
  def split(sql) do
    sql
    |> String.split("\n")
    |> Enum.map(&String.replace(&1, ~r/--.*$/, ""))
    |> Enum.join("\n")
    |> String.split(";")
    |> Enum.map(&String.trim/1)
    |> Enum.reject(&(&1 == ""))
  end
end

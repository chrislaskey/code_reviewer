defmodule EctoRust.SourceTest.Marked do
  @moduledoc false
  use Ecto.Schema
  import Ecto.Changeset

  # Source reads the mark from this file, and nothing reads the attribute.
  # Persisting it keeps Elixir from warning that it is unused.
  Module.register_attribute(__MODULE__, :universal_changeset, persist: true)

  embedded_schema do
    field :name, :string
  end

  @universal_changeset true
  def changeset(record, params, _options \\ []) do
    record
    |> cast(params, [:name])
    |> maybe(params)
    |> maybe()
  end

  defp maybe(changeset, _params), do: changeset
  defp maybe(changeset), do: changeset

  def other_changeset(record, params), do: cast(record, params, [:name])
end

defmodule EctoRust.SourceTest do
  use ExUnit.Case, async: true

  alias EctoRust.Source
  alias EctoRust.SourceTest.Marked

  test "finds the marked functions of a module with their clauses and the module's defs" do
    assert [%Source{} = source] = Source.marked(Marked)

    assert source.module == Marked
    assert source.function == :changeset
    assert source.arity == 3
    assert source.file == "test/ecto_rust/source_test.exs"
    assert source.line == 15
    assert [{:def, _, _}] = source.clauses

    assert Map.keys(source.defs) |> Enum.sort() == [
             {:changeset, 3},
             {:maybe, 1},
             {:maybe, 2},
             {:other_changeset, 2}
           ]
  end

  test "a module without a mark has none" do
    assert Source.marked(App.Data.User) == []
  end

  test "any function of a module can be read as a source" do
    assert {:ok, %Source{function: :other_changeset, arity: 2}} = Source.function(Marked, :other_changeset, 2)
    assert Source.function(Marked, :nope, 1) == :error
  end
end

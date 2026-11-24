defmodule EctoRust.TranspileTest.Branching do
  @moduledoc false
  use Ecto.Schema
  import Ecto.Changeset

  embedded_schema do
    field :name, :string
    field :count, :integer
    field :role, Ecto.Enum, values: [:member, :admin]
    field :extra, {:map, :string}
  end

  def changeset(record, params) do
    record
    |> cast(params, [:name])
    |> maybe_required(params)
  end

  defp maybe_required(changeset, %{"strict" => _}), do: validate_required(changeset, [:name])
  defp maybe_required(changeset, _params), do: changeset

  def cast_and_put(record, params) do
    record
    |> cast(params, [:name])
    |> put_change(:name, "fixed")
  end

  def validates_a_put_field(record, params) do
    record
    |> cast(params, [])
    |> put_change(:name, "fixed")
    |> validate_length(:name, max: 3)
  end

  def range(record, params) do
    record
    |> cast(params, [:count])
    |> validate_inclusion(:count, 1..3)
  end

  def with_enum(record, params) do
    record
    |> cast(params, [:role])
    |> validate_required([:role])
  end

  def uncastable(record, params) do
    cast(record, params, [:extra])
  end
end

defmodule EctoRust.TranspileTest do
  use ExUnit.Case, async: true

  alias EctoRust.{Source, Transpile}
  alias EctoRust.TranspileTest.Branching

  defp module(module, function, arity) do
    {:ok, source} = Source.function(module, function, arity)
    Transpile.module([source])
  end

  defp message(module, function, arity) do
    assert {:error, [problem]} = module(module, function, arity)
    assert problem.file =~ ~r/\.exs?$/
    assert is_integer(problem.line)
    problem.message
  end

  test "writes a pipeline of literal validations as Rust, with a helper that puts or passes" do
    assert {:ok, rust} = module(App.Data.Post, :changeset, 3)

    assert rust =~ ~s|BTreeMap::from([("body".to_string(), FieldType::String)])|
    assert rust =~ ~s|Changeset::cast(data, &fields(), params, &["body"])|
    assert rust =~ ~s|.validate_required(&["owner_id", "body"])|
    assert rust =~ ~s|.validate_length("body", &Length::default().max(10000), None)|
    assert rust =~ "/// The app sets `owner_id` and `last_synced_at`, so pass them in `data`."
    assert rust =~ "pub fn changeset_constraints() -> Vec<Constraint>"
  end

  test "writes a format with its message, and a unique constraint for the local store" do
    assert {:ok, rust} = module(App.Data.Link, :changeset, 3)
    assert rust =~ ~s|.validate_format(\n            "url",|
    assert rust =~ "must be an http:// or https:// URL"

    assert {:ok, rust} = module(App.Data.Tag, :changeset, 3)
    assert rust =~ "app_tags_owner_id_lower_name_index"
  end

  test "casts an Ecto.Enum by its values" do
    assert {:ok, rust} = module(Branching, :with_enum, 2)
    assert rust =~ ~s|FieldType::Enum(vec!["member".to_string(), "admin".to_string()])|
  end

  test "reports a helper with logic of its own, with its line" do
    assert message(App.Data.User, :email_changeset, 3) ==
             "email_changeset/3: validate_email/2 must pipe `changeset` through Ecto.Changeset functions"
  end

  test "reports a helper whose clauses give different rules" do
    assert message(Branching, :changeset, 2) ==
             "changeset/2: the clauses of maybe_required/2 give different rules"
  end

  test "reports a field that is both cast and put" do
    assert message(Branching, :cast_and_put, 2) =~ ":name is cast and put"
  end

  test "reports a validation on a field the app puts" do
    assert message(Branching, :validates_a_put_field, 2) =~
             "validate_length(:name) validates a field the app puts"
  end

  test "reports a range where Ecto keeps it in the error" do
    assert message(Branching, :range, 2) =~ "validate_inclusion takes a list here"
  end

  test "reports a cast field of a type the crate cannot cast" do
    assert message(Branching, :uncastable, 2) =~ "the crate cannot cast :extra, of type {:map, :string}"
  end
end

defmodule CodeReviewer.FunctionIndexTest do
  use ExUnit.Case, async: true

  alias CodeReviewer.FunctionIndex

  @diff """
  diff --git a/lib/a.ex b/lib/a.ex
  --- a/lib/a.ex
  +++ b/lib/a.ex
  @@ -1,7 +1,9 @@ defmodule A do
   defmodule A do
  -  def old(x), do: x
  +  def new(x), do: x
  +
     def kept(x) do
  -    x
  +    x + 1
     end
  +
  +  defp helper(a, b), do: {a, b}
   end
  """

  test "one row per changed function, sorted by module then line, with a first changed line" do
    rows = @diff |> CodeReviewer.review_diff() |> FunctionIndex.rows()

    assert Enum.map(rows, &{&1.verb, &1.visibility, &1.function, &1.line}) == [
             {:created, :public, "new/1", 2},
             {:deleted, :public, "old/1", 2},
             {:updated, :public, "kept/1", 5},
             {:created, :private, "helper/2", 8}
           ]

    assert Enum.find(rows, &(&1.function == "old/1")).side == :before
    assert Enum.all?(rows, &(&1.module == "A" and &1.file == "lib/a.ex"))
    assert rows |> Enum.map(& &1.id) |> Enum.uniq() |> length() == 4
  end

  test "summary counts verbs, visibility and lines" do
    rows = @diff |> CodeReviewer.review_diff() |> FunctionIndex.rows()
    summary = FunctionIndex.summary(rows)

    assert summary.total == 4
    assert summary.by_verb == %{created: 2, updated: 1, deleted: 1, renamed: 0}
    assert summary.by_visibility == %{public: 3, private: 1}
    assert {summary.added, summary.removed} == {3, 2}
  end

  test "by_module rolls function rows up under their module" do
    [shop] = @diff |> CodeReviewer.review_diff() |> FunctionIndex.by_module()

    assert {shop.module, shop.verb, shop.file, shop.line} == {"A", :updated, "lib/a.ex", 1}
    assert Enum.map(shop.functions, & &1.function) == ["new/1", "old/1", "kept/1", "helper/2"]
    assert shop.stats == %{added: 5, removed: 2}
  end
end

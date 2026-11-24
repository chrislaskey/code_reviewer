defmodule App.Data.Sync.MergeTest do
  use ExUnit.Case, async: true

  import App.Data.Sync.Merge

  doctest App.Data.Sync.Merge

  test "at the server's version every change applies, whatever the base" do
    assert merge(%{"body" => "b"}, 3, 3, %{}, %{"body" => "c"}) == {%{"body" => "c"}, %{}}
  end

  test "behind the server's version, fields nobody else changed apply and the others conflict" do
    current = %{"body" => "theirs", "deleted_at" => nil}
    base = %{"body" => "original", "deleted_at" => nil}
    changes = %{"body" => "mine", "deleted_at" => "2026-10-30T12:00:00.000000Z"}

    assert merge(current, 4, 2, base, changes) ==
             {%{"deleted_at" => "2026-10-30T12:00:00.000000Z"},
              %{"body" => %{"yours" => "mine", "server" => "theirs"}}}
  end

  test "a value the server already holds is neither applied nor a conflict" do
    assert merge(%{"body" => "same"}, 4, 2, %{"body" => "original"}, %{"body" => "same"}) == {%{}, %{}}
  end

  test "a change without a base value conflicts when the server's value differs" do
    assert merge(%{"body" => "theirs"}, 4, 2, %{}, %{"body" => "mine"}) ==
             {%{}, %{"body" => %{"yours" => "mine", "server" => "theirs"}}}
  end
end

defmodule CodeReviewer.FunctionDiffTest do
  use ExUnit.Case, async: true

  alias CodeReviewer.{Diff, FunctionDiff, Review}

  @before """
  defmodule A do
    def total(cart) do
      cart
    end

    def old(x), do: x
  end
  """

  @after_src """
  defmodule A do
    def total(cart) do
      cart
      |> Enum.sum()
    end

    def new(x), do: x
  end
  """

  defp file do
    dir = Path.join(System.tmp_dir!(), "fd_#{System.unique_integer([:positive])}")
    File.mkdir_p!(dir)
    File.write!(Path.join(dir, "a"), @before)
    File.write!(Path.join(dir, "b"), @after_src)
    {out, _} = System.cmd("git", ["diff", "--no-index", "--no-color", "a", "b"], cd: dir)
    File.rm_rf!(dir)
    [diff] = Diff.parse(out)
    %Review.File{path: "lib/a.ex", hunks: diff.hunks}
  end

  test "full rebuilds both sides in order with numbering" do
    lines = FunctionDiff.full(file(), @before, @after_src)

    assert Enum.map(lines, &{&1.kind, &1.old, &1.new}) == [
             {:context, 1, 1},
             {:context, 2, 2},
             {:context, 3, 3},
             {:add, nil, 4},
             {:context, 4, 5},
             {:context, 5, 6},
             {:del, 6, nil},
             {:add, nil, 7},
             {:context, 7, 8}
           ]

    assert Enum.at(lines, 3).text == "    |> Enum.sum()"
  end

  test "for_ranges keeps the whole function with its changes marked" do
    # total/1 is 2..4 before and 2..5 after
    lines = FunctionDiff.for_ranges(file(), 2..4, 2..5, @before, @after_src)

    assert Enum.map(lines, &{&1.kind, &1.text}) == [
             {:context, "  def total(cart) do"},
             {:context, "    cart"},
             {:add, "    |> Enum.sum()"},
             {:context, "  end"}
           ]
  end

  test "created and deleted functions show one side only" do
    assert [%{kind: :add, text: "  def new(x), do: x"}] =
             FunctionDiff.for_ranges(file(), nil, 7..7, @before, @after_src)

    assert [%{kind: :del, text: "  def old(x), do: x"}] =
             FunctionDiff.for_ranges(file(), 6..6, nil, @before, @after_src)
  end
end

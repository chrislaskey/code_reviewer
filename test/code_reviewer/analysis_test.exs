defmodule CodeReviewer.AnalysisTest do
  use ExUnit.Case, async: true

  alias CodeReviewer.{Analysis, Diff}

  @before """
  defmodule Shop.Cart do
    alias Shop.Repo
    alias Shop.Pricing

    def total(cart) do
      Pricing.sum(cart.items)
    end

    def add(cart, item), do: %{cart | items: [item | cart.items]}

    defp log(msg), do: IO.puts(msg)
  end
  """

  @after_src """
  defmodule Shop.Cart do
    alias Shop.Repo

    @doc "Sum of item prices with discount applied."
    def total(cart) do
      cart.items
      |> Enum.map(& &1.price)
      |> Enum.sum()
      |> discount(cart)
    end

    def add(cart, item), do: %{cart | items: [item | cart.items]}

    def remove(cart, id), do: %{cart | items: Enum.reject(cart.items, &(&1.id == id))}

    defp discount(total, %{coupon: nil}), do: total
    defp discount(total, %{coupon: c}), do: total - c.amount
  end
  """

  defp diff_text(before, after_) do
    dir = Path.join(System.tmp_dir!(), "code_reviewer_#{System.unique_integer([:positive])}")
    File.mkdir_p!(dir)
    File.write!(Path.join(dir, "before.ex"), before)
    File.write!(Path.join(dir, "after.ex"), after_)

    {out, _} =
      System.cmd("git", ["diff", "--no-index", "--no-color", "before.ex", "after.ex"],
        cd: dir,
        stderr_to_stdout: true
      )

    File.rm_rf!(dir)
    # make it look like an in-repo diff of one path
    String.replace(out, ~w(a/before.ex b/after.ex), "a/lib/shop/cart.ex")
    |> String.replace("a/lib/shop/cart.ex b/after.ex", "a/lib/shop/cart.ex b/lib/shop/cart.ex")
    |> String.replace("+++ a/lib/shop/cart.ex", "+++ b/lib/shop/cart.ex")
  end

  defp review(before, after_) do
    [diff] = Diff.parse(diff_text(before, after_))

    Analysis.analyze([diff],
      source: %{kind: :test},
      before: fn _ -> before end,
      after: fn _ -> after_ end
    )
  end

  test "classifies functions across the change with details" do
    %{files: [file]} = review(@before, @after_src)
    [cart] = file.modules

    assert file.parser == :ast
    assert cart.verb == :updated

    by_name = Map.new(cart.functions, &{&1.name, &1})

    assert by_name["total"].verb == :updated
    assert :body in by_name["total"].details
    assert :doc in by_name["total"].details

    assert by_name["add"].verb == :unchanged
    assert by_name["remove"].verb == :created
    assert by_name["log"].verb == :deleted
    assert by_name["log"].range.before == 11..11

    assert by_name["discount"].verb == :created
    assert by_name["discount"].visibility == :private
    assert by_name["discount"].clauses == %{before: 0, after: 2}
  end

  test "directives are diffed as sets" do
    %{files: [file]} = review(@before, @after_src)
    [cart] = file.modules

    assert Enum.map(cart.directives, &{&1.kind, &1.text, &1.verb}) == [
             {:alias, "Shop.Repo", :unchanged},
             {:alias, "Shop.Pricing", :deleted}
           ]
  end

  test "a module whose defmodule line changed is a rename" do
    after_ = String.replace(@after_src, "defmodule Shop.Cart", "defmodule Shop.Basket")
    %{files: [file]} = review(@before, after_)
    [m] = file.modules
    assert {m.verb, m.old_name, m.name} == {:renamed, "Shop.Cart", "Shop.Basket"}
  end

  test "without source the outline comes from the diff fragment and the header names the module" do
    [diff] = Diff.parse(diff_text(@before, @after_src))

    review =
      Analysis.analyze([diff], source: %{}, before: fn _ -> nil end, after: fn _ -> nil end)

    [file] = review.files
    [cart] = file.modules

    assert file.parser == :fragment
    assert cart.name == "Shop.Cart"

    names =
      cart.functions
      |> Enum.reject(&(&1.verb == :unchanged))
      |> Enum.map(&{&1.name, &1.verb})
      |> Enum.sort()

    assert {"remove", :created} in names
    assert {"discount", :created} in names
    assert {"log", :deleted} in names
  end

  test "non-elixir files are carried with stats but no modules" do
    text = """
    diff --git a/CHANGELOG.md b/CHANGELOG.md
    --- a/CHANGELOG.md
    +++ b/CHANGELOG.md
    @@ -1 +1,2 @@
     # Changelog
    +- Added things
    """

    [diff] = Diff.parse(text)

    %{files: [file]} =
      Analysis.analyze([diff], source: %{}, before: fn _ -> nil end, after: fn _ -> nil end)

    assert {file.language, file.parser, file.modules, file.stats} ==
             {:other, :none, [], %{added: 1, removed: 0}}
  end
end

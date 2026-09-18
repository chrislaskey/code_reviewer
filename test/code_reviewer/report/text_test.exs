defmodule CodeReviewer.Report.TextTest do
  use ExUnit.Case, async: true

  alias CodeReviewer.Report.Text

  @diff """
  diff --git a/lib/a.ex b/lib/a.ex
  --- a/lib/a.ex
  +++ b/lib/a.ex
  @@ -1,4 +1,8 @@ defmodule A do
   defmodule A do
  -  def old(x), do: x
  +  def new(x), do: x
  +
  +  defp helper(a, b) do
  +    {a, b}
  +  end
   end
  """

  test "renders every section from a diff-only review" do
    text = @diff |> CodeReviewer.review_diff(title: "demo") |> Text.render()

    assert text =~ "Change: demo"
    assert text =~ "## Files\n\nUpdated:\n - lib/a.ex"
    assert text =~ "## Modules\n\nUpdated:\n - A  lib/a.ex"
    assert text =~ "## Public Functions\n\nA (Updated)  lib/a.ex\n - Created new/1"
    assert text =~ " - Deleted old/1"
    assert text =~ "## Private Functions\n\nA (Updated)  lib/a.ex\n - Created helper/2"
    assert text =~ "## Tree\n\nlib/a.ex  Updated  +5 -1  (fragment)"
    assert text =~ "(fragment: only lines present in the diff are known)"
  end

  test "sections say (none) rather than vanishing" do
    text =
      "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-a\n+b\n"
      |> CodeReviewer.review_diff()
      |> Text.render()

    assert text =~ "## Modules\n\n(none)"
    assert text =~ "## Public Functions\n\n(none)"
  end
end

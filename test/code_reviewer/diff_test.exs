defmodule CodeReviewer.DiffTest do
  use ExUnit.Case, async: true

  alias CodeReviewer.Diff

  @updated """
  commit abc123
  Author: Someone <someone@example.com>

      Subject line

  diff --git a/lib/foo.ex b/lib/foo.ex
  index 1111111..2222222 100644
  --- a/lib/foo.ex
  +++ b/lib/foo.ex
  @@ -1,4 +1,5 @@ defmodule Foo do
   defmodule Foo do
  -  def a, do: 1
  +  def a, do: 2
  +  def b, do: 3
   end
  \\ No newline at end of file
  diff --git a/lib/new.ex b/lib/new.ex
  new file mode 100644
  index 0000000..3333333
  --- /dev/null
  +++ b/lib/new.ex
  @@ -0,0 +1,2 @@
  +defmodule New do
  +end
  diff --git a/lib/old.ex b/lib/old.ex
  deleted file mode 100644
  index 4444444..0000000
  --- a/lib/old.ex
  +++ /dev/null
  @@ -1,2 +0,0 @@
  -defmodule Old do
  -end
  diff --git a/lib/before.ex b/lib/after.ex
  similarity index 90%
  rename from lib/before.ex
  rename to lib/after.ex
  index 5555555..6666666 100644
  --- a/lib/before.ex
  +++ b/lib/after.ex
  @@ -1 +1 @@
  -defmodule Before do
  +defmodule After do
  diff --git a/priv/img.png b/priv/img.png
  index 7777777..8888888 100644
  Binary files a/priv/img.png and b/priv/img.png differ
  """

  test "skips commit headers and parses every file with its status" do
    files = Diff.parse(@updated)

    assert Enum.map(files, &{&1.old_path, &1.new_path, &1.status}) == [
             {"lib/foo.ex", "lib/foo.ex", :updated},
             {nil, "lib/new.ex", :created},
             {"lib/old.ex", nil, :deleted},
             {"lib/before.ex", "lib/after.ex", :renamed},
             {"priv/img.png", "priv/img.png", :updated}
           ]

    assert List.last(files).binary
    assert List.last(files).hunks == []
    assert Enum.at(files, 3).similarity == 90
  end

  test "numbers hunk lines on both sides and drops the no-newline marker" do
    [foo | _] = Diff.parse(@updated)
    [hunk] = foo.hunks

    assert hunk.header == "defmodule Foo do"
    assert {hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count} == {1, 4, 1, 5}

    assert hunk.lines == [
             {:context, 1, 1, "defmodule Foo do"},
             {:del, 2, nil, "  def a, do: 1"},
             {:add, nil, 2, "  def a, do: 2"},
             {:add, nil, 3, "  def b, do: 3"},
             {:context, 3, 4, "end"}
           ]

    assert Diff.stats(foo) == %{added: 2, removed: 1}
  end

  test "hunk headers without a count default to one line" do
    [_, _, _, renamed, _] = Diff.parse(@updated)
    [hunk] = renamed.hunks
    assert {hunk.old_count, hunk.new_count} == {1, 1}
  end

  test "fragment rebuilds one side with a map back to real line numbers" do
    [foo | _] = Diff.parse(@updated)

    {before, before_map} = Diff.fragment(foo, :before)
    assert before == "defmodule Foo do\n  def a, do: 1\nend"
    assert before_map == %{1 => 1, 2 => 2, 3 => 3}

    {after_text, after_map} = Diff.fragment(foo, :after)
    assert after_text == "defmodule Foo do\n  def a, do: 2\n  def b, do: 3\nend"
    assert after_map == %{1 => 1, 2 => 2, 3 => 3, 4 => 4}
  end

  test "a hunk whose context lines were emitted as empty strings still parses" do
    text = """
    diff --git a/x.ex b/x.ex
    --- a/x.ex
    +++ b/x.ex
    @@ -1,3 +1,3 @@
     a

    -b
    +c
    """

    [file] = Diff.parse(text)
    [hunk] = file.hunks
    assert Enum.map(hunk.lines, &elem(&1, 0)) == [:context, :context, :del, :add]
  end
end

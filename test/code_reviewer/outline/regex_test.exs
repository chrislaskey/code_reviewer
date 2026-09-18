defmodule CodeReviewer.Outline.RegexTest do
  use ExUnit.Case, async: true

  alias CodeReviewer.Outline

  # Every case here is also run through the AST parser so the two stay in
  # agreement; see `both/1`.

  defp both(source) do
    regex = Outline.Regex.parse(source)
    {:ok, ast} = Outline.AST.parse(source)
    assert strip(regex) == strip(ast), "regex and AST outlines differ"
    regex
  end

  defp strip(modules) do
    for m <- modules do
      {m.name, m.line_start, m.line_end,
       for(
         f <- m.functions,
         do: {f.kind, f.name, f.arity, f.visibility, f.line_start, f.line_end, length(f.clauses)}
       )}
    end
  end

  defp funs(module),
    do: Enum.map(module.functions, &{&1.kind, &1.name, &1.arity, &1.line_start, &1.line_end})

  test "module range, public and private functions with arity" do
    [m] =
      both("""
      defmodule Foo.Bar do
        def zero, do: :ok
        def one(a), do: a

        def two(a, b) do
          a + b
        end

        defp helper(x), do: x
      end
      """)

    assert m.name == "Foo.Bar"
    assert {m.line_start, m.line_end} == {1, 10}

    assert funs(m) == [
             {:def, "zero", 0, 2, 2},
             {:def, "one", 1, 3, 3},
             {:def, "two", 2, 5, 7},
             {:defp, "helper", 1, 9, 9}
           ]

    assert Enum.map(m.functions, & &1.visibility) == [:public, :public, :public, :private]
  end

  test "clauses of the same name and arity are grouped, ranges span all of them" do
    [m] =
      both("""
      defmodule Foo do
        def f(nil), do: nil
        def f(%{a: a}) when a > 1, do: a

        def f(other) do
          other
        end
      end
      """)

    [f] = m.functions
    assert {f.name, f.arity, length(f.clauses)} == {"f", 1, 3}
    assert {f.line_start, f.line_end} == {2, 7}
  end

  test "multi-line heads with guards, defaults, and question-mark names" do
    [m] =
      both("""
      defmodule Foo do
        def status_allows?(
              %Flow{status: "pre_release"},
              _action,
              %{ids: ids} = assigns
            )
            when is_list(ids),
            do: assigns.user_id in ids

        def with_default(a, b \\\\ [], c \\\\ %{}), do: {a, b, c}

        defp next_up(rows, continue_allowed?) do
          continue_allowed? && rows
        end
      end
      """)

    assert funs(m) == [
             {:def, "status_allows?", 3, 2, 8},
             {:def, "with_default", 3, 10, 10},
             {:defp, "next_up", 2, 12, 14}
           ]
  end

  test "rescue, catch and after at def indentation do not end the clause" do
    [m] =
      both("""
      defmodule Foo do
        defp parse(socket, version) do
          assign(socket, parsed: version)
        rescue
          error -> assign(socket, error: error)
        end

        def other, do: :ok
      end
      """)

    assert funs(m) == [{:defp, "parse", 2, 2, 6}, {:def, "other", 0, 8, 8}]
  end

  test "heredocs and comments containing def are ignored" do
    [m] =
      both(~S'''
      defmodule Foo do
        @moduledoc """
        def not_a_function do
        end
        """

        # def also_not_a_function, do: :ok

        def render(assigns) do
          ~H"""
          <p>def nope</p>
          """
        end
      end
      ''')

    assert funs(m) == [{:def, "render", 1, 9, 13}]
  end

  test "defs inside a quote block belong to the enclosing macro, not the module" do
    [m] =
      both("""
      defmodule Foo do
        defmacro __using__(_opts) do
          quote do
            def injected, do: :ok
          end
        end
      end
      """)

    assert funs(m) == [{:defmacro, "__using__", 1, 2, 6}]
  end

  test "nested modules get qualified names and their own functions" do
    modules =
      both("""
      defmodule Outer do
        def a, do: 1

        defmodule Inner do
          def b, do: 2
        end

        defmodule __MODULE__.Deep do
          def c, do: 3
        end
      end
      """)

    assert Enum.map(modules, &{&1.name, Enum.map(&1.functions, fn f -> f.name end)}) == [
             {"Outer", ["a"]},
             {"Outer.Inner", ["b"]},
             {"Outer.Deep", ["c"]}
           ]
  end

  test "defdelegate, defguard and defmacro kinds" do
    [m] =
      both("""
      defmodule Foo do
        defdelegate decode(ctx, token, opts \\\\ []),
          to: Other,
          as: :dec

        defguardp writable?(socket)
                  when socket.assigns.ok? and true

        defmacro m(a), do: a
        defmacrop mp(a, b), do: {a, b}
      end
      """)

    assert funs(m) == [
             {:defdelegate, "decode", 3, 2, 4},
             {:defguardp, "writable?", 1, 6, 7},
             {:defmacro, "m", 1, 9, 9},
             {:defmacrop, "mp", 2, 10, 10}
           ]
  end

  test "test blocks are members named by their description, including inside describe" do
    [m] =
      both(~S'''
      defmodule FooTest do
        use ExUnit.Case

        test "plain" do
          assert true
        end

        describe "group" do
          test "with a \"quoted\" word", %{conn: conn} do
            assert conn
          end

          defp helper, do: :ok
        end
      end
      ''')

    assert funs(m) == [
             {:test, "plain", nil, 4, 6},
             {:test, "with a \"quoted\" word", nil, 9, 11},
             {:defp, "helper", 0, 13, 13}
           ]
  end

  test "directives and annotations are captured" do
    [m] =
      both("""
      defmodule Foo do
        @moduledoc "Docs here"
        use Phoenix.Component
        alias Foo.Bar, as: Baz
        import Enum, only: [map: 2]

        attr(:name, :string, required: true)

        @doc "adds"
        @spec add(integer(), integer()) :: integer()
        @impl true
        def add(a, b), do: a + b
      end
      """)

    assert Enum.map(m.directives, & &1.kind) == [:moduledoc, :use, :alias, :import, :attr]
    assert Enum.find(m.directives, &(&1.kind == :alias)).text =~ "Foo.Bar"

    [add] = m.functions
    assert Enum.map(add.annotations, & &1.kind) == [:"@doc", :"@spec", :"@impl"]
  end

  test "a fragment without defmodule still yields functions under a nameless module" do
    [m] =
      Outline.Regex.parse("""
        def start(socket), do: socket

        def reopen(%{flow: flow} = assigns) do
          flow
        end

        defp start_instance(a, b, c) do
      """)

    assert m.name == nil

    assert Enum.map(m.functions, &{&1.name, &1.arity}) == [
             {"start", 1},
             {"reopen", 1},
             {"start_instance", 3}
           ]
  end

  test "Outline.parse falls back to regex when the source does not parse" do
    complete = "defmodule A do\n  def a, do: 1\nend\n"
    broken = "defmodule A do\n  def a, do: 1\n"

    assert %Outline{parser: :ast} = Outline.parse(complete)
    assert %Outline{parser: :regex, modules: [%{name: "A"}]} = Outline.parse(broken)
  end
end

defmodule App.Data.UniversalChangesetTest do
  # A changeset function marked @universal_changeset true runs Rust, and its
  # Ecto pipeline stays in the source as the template the Rust was written
  # from. On every case here the two must give the same errors. Add a case
  # for every rule a marked function gains.
  use App.DataCase, async: true

  alias App.Data.{Comment, Link, Post, PostTag, Tag, UniversalChangeset, UserProfile}

  @owner Ecto.UUID.generate()
  @long String.duplicate("x", 10_001)

  @cases [
    %{function: :changeset, name: "a post", params: %{"body" => "Hello"}},
    %{function: :changeset, name: "no params", params: %{}},
    %{function: :changeset, name: "no owner", params: %{"body" => "Hello"}, options: []},
    %{function: :changeset, name: "blank body", params: %{"body" => "  "}},
    %{function: :changeset, name: "body too long", params: %{"body" => @long}},
    %{function: :changeset, name: "body at the limit", params: %{"body" => String.duplicate("x", 10_000)}},
    %{function: :changeset, name: "body not a string", params: %{"body" => 1}},
    %{function: :changeset, name: "atom keys", params: %{body: "Hello"}},
    %{
      function: :changeset,
      name: "editing keeps the owner",
      data: %{owner_id: @owner, body: "Old"},
      params: %{"body" => "New"},
      options: []
    },
    %{
      function: :changeset,
      name: "an unchanged body is not validated",
      data: %{owner_id: @owner, body: @long},
      params: %{"body" => @long},
      options: []
    },
    %{
      module: UserProfile,
      function: :changeset,
      name: "a profile",
      params: %{"display_name" => "Ada", "bio" => "Hi"}
    },
    %{module: UserProfile, function: :changeset, name: "an empty profile", params: %{}},
    %{
      module: UserProfile,
      function: :changeset,
      name: "no user",
      params: %{"display_name" => "Ada"},
      options: []
    },
    %{
      module: UserProfile,
      function: :changeset,
      name: "display name too long",
      params: %{"display_name" => String.duplicate("x", 101)}
    },
    %{
      module: UserProfile,
      function: :changeset,
      name: "bio too long",
      params: %{"bio" => String.duplicate("x", 1001)}
    },
    %{
      module: UserProfile,
      function: :changeset,
      name: "editing keeps the user",
      data: %{user_id: @owner, display_name: "Old"},
      params: %{"display_name" => "New"},
      options: []
    },
    %{
      module: Link,
      function: :changeset,
      name: "a link",
      params: %{"post_id" => @owner, "label" => "Docs", "url" => "https://hexdocs.pm/ecto"}
    },
    %{module: Link, function: :changeset, name: "an empty link", params: %{}},
    %{
      module: Link,
      function: :changeset,
      name: "a URL that is not http",
      params: %{"post_id" => @owner, "label" => "Docs", "url" => "ftp://example.com"}
    },
    %{
      module: Link,
      function: :changeset,
      name: "a URL with no host",
      params: %{"post_id" => @owner, "label" => "Docs", "url" => "https://"}
    },
    %{
      module: Link,
      function: :changeset,
      name: "a URL with a space",
      params: %{"post_id" => @owner, "label" => "Docs", "url" => "https://example.com/a b"}
    },
    %{
      module: Link,
      function: :changeset,
      name: "label and URL too long",
      params: %{
        "post_id" => @owner,
        "label" => String.duplicate("x", 101),
        "url" => "https://example.com/" <> String.duplicate("x", 2000)
      }
    },
    %{module: Link, function: :changeset, name: "a post id that is not a UUID", params: %{"post_id" => "1"}},
    %{module: Tag, function: :changeset, name: "a tag", params: %{"name" => "elixir"}},
    %{module: Tag, function: :changeset, name: "no owner", params: %{"name" => "elixir"}, options: []},
    %{module: Tag, function: :changeset, name: "a blank name", params: %{"name" => " "}},
    %{
      module: Tag,
      function: :changeset,
      name: "a name too long",
      params: %{"name" => String.duplicate("x", 51)}
    },
    %{
      module: PostTag,
      function: :changeset,
      name: "a tagging",
      params: %{"post_id" => @owner, "tag_id" => @owner, "position" => 0, "note" => "why"}
    },
    %{module: PostTag, function: :changeset, name: "an empty tagging", params: %{}},
    %{
      module: PostTag,
      function: :changeset,
      name: "a negative position and a note too long",
      params: %{
        "post_id" => @owner,
        "tag_id" => @owner,
        "position" => -1,
        "note" => String.duplicate("x", 201)
      }
    },
    %{
      module: PostTag,
      function: :changeset,
      name: "a position from a form",
      params: %{"post_id" => @owner, "tag_id" => @owner, "position" => "2"}
    },
    %{
      module: PostTag,
      function: :changeset,
      name: "a position that is not a number",
      params: %{"post_id" => @owner, "tag_id" => @owner, "position" => "first"}
    },
    %{
      module: Comment,
      function: :changeset,
      name: "a comment",
      params: %{"posts_communities_id" => @owner, "body" => "Hi"}
    },
    %{module: Comment, function: :changeset, name: "an empty comment", params: %{}},
    %{
      module: Comment,
      function: :changeset,
      name: "no commenter",
      params: %{"posts_communities_id" => @owner, "body" => "Hi"},
      options: []
    },
    %{
      module: Comment,
      function: :changeset,
      name: "a body too long",
      params: %{"posts_communities_id" => @owner, "body" => String.duplicate("x", 2001)}
    },
    %{
      module: Comment,
      function: :changeset,
      name: "editing keeps the commenter and the thread",
      data: %{posts_communities_id: @owner, owner_id: @owner, body: "Old"},
      params: %{"body" => "New"},
      options: []
    }
  ]

  defp cases do
    for case <- @cases do
      %{module: Post, data: %{}, options: [owner_id: @owner, user_id: @owner]} |> Map.merge(case)
    end
  end

  defp args(%{module: module, data: data, params: params, options: options}),
    do: [struct(module, data), params, options]

  test "the marked functions are the ones with cases here" do
    marked = for {module, sources} <- EctoRust.marked(:app), source <- sources, do: {module, source.function}
    with_cases = cases() |> Enum.map(&{&1.module, &1.function}) |> Enum.uniq()

    assert Enum.sort(marked) == Enum.sort(with_cases),
           "a marked function needs cases here, and a function no longer marked loses its cases"
  end

  test "the Rust gives the errors the Ecto template gives" do
    differences =
      for case <- cases(),
          template = UniversalChangeset.template(case.module, case.function, args(case)),
          rust = apply(case.module, case.function, args(case)),
          {errors(template), template.valid?} != {errors(rust), rust.valid?} do
        "#{inspect(case.module)}.#{case.function} #{case.name}:\n  template: #{inspect(errors(template))}\n  rust:     #{inspect(errors(rust))}"
      end

    assert differences == [], Enum.join(differences, "\n")
  end

  test "the changeset keeps what the template puts and what the repo reads" do
    changeset = Post.changeset(%Post{}, %{"body" => "Hello"}, owner_id: @owner)

    assert changeset.valid?
    assert changeset.changes.body == "Hello"
    assert changeset.changes.owner_id == @owner
    assert %DateTime{} = changeset.changes.last_synced_at
    assert changeset.required == [:owner_id, :body]
    assert [{:body, {:length, [max: 10_000]}}] = changeset.validations
  end

  test "an error is what Ecto reports, template and options included" do
    changeset = Post.changeset(%Post{}, %{"body" => @long}, owner_id: @owner)

    assert changeset.errors == [
             body:
               {"should be at most %{count} character(s)",
                [count: 10_000, validation: :length, kind: :max, type: :string]}
           ]

    assert errors_on(changeset) == %{body: ["should be at most 10000 character(s)"]}
    refute changeset.valid?
  end

  # Errors of one validate_required call keep their field order in Ecto
  # and come reversed from the Rust, so they compare sorted
  defp errors(changeset), do: Enum.sort(changeset.errors)
end

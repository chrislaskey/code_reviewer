defmodule App.Data.PostsFixtures do
  @moduledoc """
  Test helpers for posts, via `App.Data.Posting`.
  """

  alias App.Data.Posting

  @doc """
  A post by the scope's user, in the communities with the given ids.
  """
  def post_fixture(scope, community_ids \\ [], attrs \\ %{}) do
    {:ok, post} =
      Posting.write(scope, Enum.into(attrs, %{body: "post #{System.unique_integer()}"}), community_ids)

    post
  end
end

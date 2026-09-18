defmodule CodeReviewerTest do
  use ExUnit.Case
  doctest CodeReviewer

  test "greets the world" do
    assert CodeReviewer.hello() == :world
  end
end

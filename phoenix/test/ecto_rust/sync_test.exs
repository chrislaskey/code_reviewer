defmodule EctoRust.SyncTest do
  # Fails when a function marked @universal_changeset true and its .rs file,
  # the NIF crate or the module that loads it have drifted apart, the same
  # check as `mix app.rust --check`
  use ExUnit.Case, async: true

  test "every @universal_changeset function's Rust and the NIF are up to date" do
    problems = EctoRust.check(EctoRust.marked(:app))

    assert problems == [], Enum.map_join(problems, "\n", &EctoRust.describe/1)
  end
end

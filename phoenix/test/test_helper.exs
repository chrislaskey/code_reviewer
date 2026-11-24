# The code generator tests fail when the project sits under a folder whose
# name starts with a dot, such as a Claude Code worktree in .claude/, since
# igniter's file lookup skips those folders. CI runs them, see
# MIX_TEST_INCLUDE in .github/workflows/protected-check.yml. Locally run them
# with `mix test --include code_generators`
#
# `record_cases` writes what the dynamic_form library does into
# native/dynamic_form/tests/cases.json, see dynamic_form_cases_test.exs
ExUnit.start(exclude: [:code_generators, :record_cases])
Ecto.Adapters.SQL.Sandbox.mode(App.Repo, :manual)

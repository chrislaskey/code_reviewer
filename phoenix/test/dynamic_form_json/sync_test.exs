defmodule DynamicFormJson.SyncTest do
  # Fails when a function marked @universal_dynamic_form and its .json file
  # have drifted apart, the same check as `mix app.dynamic_forms --check`
  use ExUnit.Case, async: true

  test "every @universal_dynamic_form function's file is up to date" do
    {forms, problems} = DynamicFormJson.forms(:app)
    problems = problems ++ DynamicFormJson.check(forms) ++ DynamicFormJson.orphans(forms)

    assert problems == [], Enum.map_join(problems, "\n", &DynamicFormJson.describe/1)
  end
end

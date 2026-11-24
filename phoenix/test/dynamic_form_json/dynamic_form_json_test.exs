defmodule DynamicFormJsonTest do
  use ExUnit.Case, async: true

  alias DynamicFormJson.Source

  defmodule Forms do
    Module.register_attribute(__MODULE__, :universal_dynamic_form, accumulate: true)

    @universal_dynamic_form true
    def name_form do
      %{
        "id" => "name-form",
        "elements" => [%{"type" => "text", "name" => "name", "title" => "Name", "isRequired" => true}]
      }
    end

    # Another arity is another function, and unmarked
    def name_form(title), do: put_in(name_form(), ["elements", Access.at(0), "title"], title)

    @universal_dynamic_form true
    def atom_form, do: %{"elements" => [%{"type" => :text, "name" => "name"}]}

    @universal_dynamic_form true
    def unreadable_form, do: %{"elements" => [%{"type" => "text"}]}

    @universal_dynamic_form true
    def with_argument_form(choices), do: %{"elements" => choices}

    @universal_dynamic_form true
    defp private_form, do: %{"elements" => []}

    def unmarked_form, do: private_form()
  end

  setup_all do
    {forms, errors} = Source.forms(Forms)
    %{forms: Map.new(forms, &{&1.function, &1}), errors: errors}
  end

  test "finds the marked functions and refuses the ones it can't call", %{forms: forms, errors: errors} do
    assert forms |> Map.keys() |> Enum.sort() == [:atom_form, :name_form, :unreadable_form]

    assert [
             %{line: 27, message: "with_argument_form: a marked form takes no arguments" <> _},
             %{line: 30, message: "private_form: a marked form is public" <> _}
           ] = errors
  end

  test "writes the definition as JSON with sorted keys, next to the module", %{forms: forms} do
    assert DynamicFormJson.path(forms.name_form) == "test/dynamic_form_json/name_form.dynamic_form.json"

    assert DynamicFormJson.render(forms.name_form) ==
             {:ok,
              """
              {
                "elements": [
                  {
                    "isRequired": true,
                    "name": "name",
                    "title": "Name",
                    "type": "text"
                  }
                ],
                "id": "name-form"
              }
              """}
  end

  test "refuses a definition that isn't the same data as JSON", %{forms: forms} do
    assert {:error, %{message: "DynamicFormJsonTest.Forms.atom_form/0: returns JSON data only" <> _}} =
             DynamicFormJson.render(forms.atom_form)
  end

  test "refuses a definition the library can't read", %{forms: forms} do
    assert {:error,
            %{message: "DynamicFormJsonTest.Forms.unreadable_form/0: dynamic_form can't read it" <> _}} =
             DynamicFormJson.render(forms.unreadable_form)
  end

  test "reports a missing file, and no orphans for a pattern with none", %{forms: forms} do
    assert DynamicFormJson.check([forms.name_form]) == [
             {:missing, "test/dynamic_form_json/name_form.dynamic_form.json"}
           ]

    assert DynamicFormJson.orphans([forms.name_form], "test/dynamic_form_json/*.dynamic_form.json") == []
  end
end

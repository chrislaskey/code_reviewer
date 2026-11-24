defmodule App.Web.DynamicForms do
  @moduledoc """
  Helpers for the `dynamic_form` definitions a LiveView defines beside
  itself. A definition marked `@universal_dynamic_form true` is static and
  written to a `.json` file for the desktop app, see
  `mix help app.dynamic_forms`; what only the server knows at render time,
  like the user's communities as choices, is filled in here, and by the
  desktop app the same way. See `docs/design/dynamic-forms.md`.
  """

  @doc """
  Sets the choices of the question called `name`, as maps of `"value"`
  and `"text"`.
  """
  def put_choices(definition, name, choices) do
    Map.update!(definition, "elements", fn elements ->
      Enum.map(elements, fn
        %{"name" => ^name} = element -> Map.put(element, "choices", choices)
        element -> element
      end)
    end)
  end
end

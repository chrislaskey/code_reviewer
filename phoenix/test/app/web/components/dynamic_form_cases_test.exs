defmodule App.Web.DynamicFormCasesTest do
  # The desktop app shows and validates forms with the Rust in
  # native/dynamic_form, which follows the dynamic_form library. Its
  # tests/cases.json is what the library does, and both hold to it: this
  # test holds the library, `cargo test` the Rust.
  #
  # - `cases`: the errors for params, a nested entry's under
  #   `name.index.field`, and the changes when a case lists them
  # - `expressions`: what a condition evaluates to over raw params
  # - `views`: an outline of the rendered form, one line per control,
  #   error, heading, note and button, read here from the HTML the library
  #   renders through App.Web.CoreComponents
  # - `flows`: the form's lifecycle, from data through changes, entries
  #   added and removed, and submits, with the outline after each step,
  #   run here through `DynamicForm.form` in App.DynamicFormFlowLive. An
  #   entry's generated id reads as `<id>`
  #
  # To add a case, write its input in the file with `[]` or `null` for
  # what it expects, run `mix test --only record_cases` to fill that in
  # from the library, read what it wrote, then make `cargo test` pass.
  use ExUnit.Case, async: true

  import ExUnit.CaptureLog
  import Phoenix.ConnTest, only: [build_conn: 0]
  import Phoenix.LiveViewTest

  @endpoint App.Web.Endpoint

  alias App.Web.PostsLive
  alias DynamicForm.Instance.Elements

  @cases_path "native/dynamic_form/tests/cases.json"
  @external_resource @cases_path
  @cases @cases_path |> File.read!() |> Jason.decode!()

  test "the library gives the errors in cases.json" do
    differences =
      Enum.flat_map(@cases["cases"], fn case ->
        library = case_result(case)

        expected = %{
          "errors" => Enum.sort_by(case["errors"], &Jason.encode!/1),
          "changes" => case["changes"]
        }

        # A case without changes checks only the errors
        library = if Map.has_key?(case, "changes"), do: library, else: Map.delete(library, "changes")
        expected = Map.reject(expected, fn {_key, value} -> is_nil(value) end)
        difference(case["name"], expected, library)
      end)

    assert differences == [], Enum.join(differences, "\n")
  end

  test "the library evaluates the conditions in cases.json" do
    differences =
      Enum.flat_map(@cases["expressions"], fn case ->
        difference(case["expression"], case["expected"], evaluate(case))
      end)

    assert differences == [], Enum.join(differences, "\n")
  end

  test "the library renders the views in cases.json" do
    differences =
      Enum.flat_map(@cases["views"], fn case ->
        difference(case["name"], case["outline"], outline(case))
      end)

    assert differences == [], Enum.join(differences, "\n")
  end

  test "the library runs the flows in cases.json" do
    differences =
      Enum.flat_map(@cases["flows"], fn case ->
        difference(case["name"], case["outlines"], run_flow(case))
      end)

    assert differences == [], Enum.join(differences, "\n")
  end

  test "the post form in cases.json is the app's" do
    app = PostsLive.post_form()

    assert Map.delete(app, "id") == @cases["definitions"]["post_form"]
  end

  test "the post form offers the communities given" do
    instance = PostsLive.post_form([%{id: "c1", name: "Gardeners"}])
    question = Enum.find(instance.elements, &(&1.name == "community_ids"))

    assert question.choices == [{"Gardeners", "c1"}]
  end

  @tag :record_cases
  test "records what the library does into cases.json" do
    cases =
      Map.merge(@cases, %{
        "cases" =>
          Enum.map(@cases["cases"], fn case ->
            result = case_result(case)
            case = Map.put(case, "errors", result["errors"])
            if Map.has_key?(case, "changes"), do: Map.put(case, "changes", result["changes"]), else: case
          end),
        "expressions" => Enum.map(@cases["expressions"], &Map.put(&1, "expected", evaluate(&1))),
        "views" => Enum.map(@cases["views"], &Map.put(&1, "outline", outline(&1))),
        "flows" => Enum.map(@cases["flows"], &Map.put(&1, "outlines", run_flow(&1)))
      })

    File.write!(@cases_path, Jason.encode_to_iodata!(cases, pretty: true))
  end

  defp difference(_name, expected, expected), do: []

  defp difference(name, expected, library) do
    ["#{name}:\n  cases.json: #{Jason.encode!(expected)}\n  library:    #{Jason.encode!(library)}"]
  end

  defp instance(case), do: DynamicForm.Parser.FromData.parse!(@cases["definitions"][case["definition"]])

  defp case_result(case) do
    instance = instance(case)
    changeset = DynamicForm.Changeset.create_changeset(instance, case["params"])
    questions = Elements.list_questions(instance.elements)

    errors =
      changeset
      |> errors("")
      |> Kernel.++(entry_errors(questions, changeset.params, [root_questions: questions], ""))
      |> Enum.sort_by(&Jason.encode!/1)

    %{"errors" => errors, "changes" => json(changeset.changes)}
  end

  # The entries' changesets as the library builds them when it validates,
  # their errors under `name.index.`, recursively
  defp entry_errors(questions, params, opts, prefix) do
    for question <- questions, question.type == "paneldynamic", reduce: [] do
      errors ->
        children = DynamicForm.NestedForms.list_entry_changesets(question, params || %{}, opts)

        errors ++
          (children
           |> Enum.with_index()
           |> Enum.flat_map(fn {child, index} ->
             child_prefix = "#{prefix}#{question.name}.#{index}."
             template = Elements.list_questions(question.templateElements || [])
             errors(child, child_prefix) ++ entry_errors(template, child.params, opts, child_prefix)
           end))
    end
  end

  defp evaluate(case) do
    capture_log(fn ->
      send(
        self(),
        {:result, DynamicForm.Visibility.evaluate_expression(case["expression"], case["params"] || %{})}
      )
    end)

    assert_received {:result, result}
    result
  end

  defp outline(case) do
    instance = instance(case)

    changeset =
      instance
      |> DynamicForm.Changeset.create_changeset(case["params"] || %{})
      |> Map.put(:action, if(case["submitted"], do: :validate))

    render_component(&DynamicForm.Renderer.Component.render/1,
      instance: instance,
      form: Phoenix.Component.to_form(changeset, as: "f"),
      form_id: "f",
      components: App.Web.CoreComponents
    )
    |> LazyHTML.from_fragment()
    |> LazyHTML.query("input, select, textarea, p, h2, h3, h4, button")
    |> Enum.flat_map(&outline_line/1)
  end

  # The outline as the form first shows and after each step, and after a
  # submit, the data it sent the LiveView or `invalid`
  defp run_flow(case) do
    session = %{"definition" => @cases["definitions"][case["definition"]], "data" => case["data"] || %{}}
    {:ok, view, _html} = live_isolated(build_conn(), App.DynamicFormFlowLive, session: session)

    [flow_outline(view) | Enum.map(case["steps"], &flow_step(view, &1))]
  end

  defp flow_step(view, %{"change" => params}) do
    view |> element("#f-form") |> render_change(%{"f" => params})
    flow_outline(view)
  end

  defp flow_step(view, %{"add" => path}) do
    view
    |> element(~s(#f-form button[phx-click="add_nested_entry"][phx-value-path="#{path}"]))
    |> render_click()

    flow_outline(view)
  end

  defp flow_step(view, %{"remove" => [path, index]}) do
    view
    |> element(~s(#f-form button[phx-value-path="#{path}"][phx-value-index="#{index}"]))
    |> render_click()

    flow_outline(view)
  end

  defp flow_step(view, %{"submit" => params}) do
    {before, _} = flow_result(view)
    view |> element("#f-form") |> render_submit(%{"f" => params})
    {successes, data} = flow_result(view)
    flow_outline(view) ++ ["result " <> if(successes == before, do: "invalid", else: ids(data))]
  end

  defp flow_result(view) do
    [successes | data] =
      view
      |> element("#result")
      |> render()
      |> LazyHTML.from_fragment()
      |> LazyHTML.text()
      |> String.split(" ", parts: 2)

    {successes, Enum.join(data)}
  end

  defp flow_outline(view) do
    view
    |> element("#f-form")
    |> render()
    |> LazyHTML.from_fragment()
    |> LazyHTML.query("input, select, textarea, p, h2, h3, h4, button")
    |> Enum.flat_map(&outline_line/1)
    |> Enum.map(&ids/1)
  end

  defp ids(text),
    do: Regex.replace(~r/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/, text, "<id>")

  # One line for a node of the rendered form, or none
  defp outline_line(node) do
    [tag] = LazyHTML.tag(node)
    outline_line(tag, node, &attribute(node, &1))
  end

  defp outline_line("input", node, attribute) do
    case attribute.("name") do
      "_csrf_token" ->
        []

      name ->
        type = attribute.("type") || "text"
        checked = if type in ["checkbox", "radio"], do: flag(node, "checked"), else: ""
        ["input #{type} #{name}=#{attribute.("value")}#{checked}#{flags(node, ["disabled", "readonly"])}"]
    end
  end

  defp outline_line("select", node, attribute) do
    selected = node |> LazyHTML.query("option[selected]") |> LazyHTML.attribute("value") |> Enum.join(",")
    ["select #{attribute.("name")} [#{selected}]#{flag(node, "disabled")}"]
  end

  defp outline_line("textarea", node, attribute) do
    value = String.trim_leading(LazyHTML.text(node), "\n")
    ["textarea #{attribute.("name")}=#{value}#{flags(node, ["disabled", "readonly"])}"]
  end

  defp outline_line("button", node, attribute) do
    text = squish(LazyHTML.text(node))
    ["button #{if text == "", do: attribute.("aria-label"), else: text}"]
  end

  defp outline_line("p", node, attribute) do
    text = squish(LazyHTML.text(node))
    if (attribute.("class") || "") =~ "text-error", do: ["error #{text}"], else: ["p #{text}"]
  end

  defp outline_line(heading, node, _attribute), do: ["#{heading} #{squish(LazyHTML.text(node))}"]

  defp attribute(node, name), do: node |> LazyHTML.attribute(name) |> List.first()
  defp flag(node, name), do: if(attribute(node, name), do: " #{name}", else: "")
  defp flags(node, names), do: Enum.map_join(names, &flag(node, &1))

  defp squish(text), do: text |> String.split() |> Enum.join(" ")

  # Each error as `[field, message, [[option, value], ...]]`, as the Rust
  # test writes it: atoms as strings, `{:array, :string}` as
  # `{"array": "string"}`, and a Decimal, which Ecto keeps for a number
  # question's bound where the Rust keeps a float, as a number
  defp errors(changeset, prefix) do
    Enum.map(changeset.errors, fn {field, {message, options}} ->
      [
        "#{prefix}#{field}",
        message,
        Enum.map(options, fn {key, value} -> [Atom.to_string(key), json(value)] end)
      ]
    end)
  end

  defp json(value) when is_boolean(value) or is_nil(value), do: value
  defp json(value) when is_atom(value), do: Atom.to_string(value)
  defp json({:array, inner}), do: %{"array" => json(inner)}

  defp json(%Decimal{} = value),
    do: if(Decimal.integer?(value), do: Decimal.to_integer(value), else: Decimal.to_float(value))

  defp json(map) when is_map(map), do: Map.new(map, fn {key, value} -> {to_string(key), json(value)} end)
  defp json(list) when is_list(list), do: Enum.map(list, &json/1)
  defp json(value), do: value
end

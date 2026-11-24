defmodule App.Web.DynamicFormComponentsTest do
  # dynamic_form renders every question through App.Web.CoreComponents'
  # input/1, as the app's own forms do. The markup here is what the desktop
  # app renders from the props its Rust gives the form field components'
  # .svelte mirrors, see native/dynamic_form/src/view.rs and its tests.
  use ExUnit.Case, async: true

  import Phoenix.LiveViewTest

  @definition %{
    "elements" => [
      %{"name" => "body", "type" => "comment", "title" => "Write a post", "isRequired" => true},
      %{"name" => "tags", "type" => "checkbox", "choices" => ["a", %{"value" => "b", "text" => "Bee"}]},
      %{"name" => "kind", "type" => "dropdown", "choices" => ["x"], "description" => "Pick one"},
      %{"name" => "labels", "type" => "tagbox", "choices" => ["x", "y"]},
      %{"name" => "score", "type" => "rating", "rateMax" => 3},
      %{"name" => "size", "type" => "radiogroup", "choices" => ["s", "m"]},
      %{"name" => "agree", "type" => "boolean", "title" => false},
      %{"name" => "email", "type" => "text", "inputType" => "email", "placeholder" => "you@example.com"}
    ]
  }

  defp render_form(params, opts \\ []) do
    instance = DynamicForm.Parser.FromData.parse!(@definition)

    changeset =
      instance
      |> DynamicForm.Changeset.create_changeset(params)
      |> Map.put(:action, :validate)

    render_component(&DynamicForm.Renderer.Component.render/1,
      instance: instance,
      form: Phoenix.Component.to_form(changeset, as: "post"),
      form_id: "post-form",
      components: App.Web.CoreComponents,
      submit_text: "Post",
      disabled: Keyword.get(opts, :disabled, false)
    )
    |> LazyHTML.from_fragment()
  end

  defp attribute(html, selector, name) do
    html |> LazyHTML.query(selector) |> LazyHTML.attribute(name)
  end

  test "a comment is a textarea with its label, required mark and errors" do
    html = render_form(%{"body" => ""})

    assert attribute(html, "textarea#post-form_body", "name") == ["post[body]"]
    assert attribute(html, "textarea#post-form_body", "rows") == ["4"]
    assert LazyHTML.text(LazyHTML.query(html, "label[for=post-form_body]")) =~ "Write a post*"
    assert LazyHTML.text(html) =~ "can't be blank"
  end

  test "a field's errors wait until it is used" do
    html = render_form(%{})

    refute LazyHTML.text(html) =~ "can't be blank"
  end

  test "choices render as checkboxes, a select or radio buttons, marked from the value" do
    html =
      render_form(%{"tags" => ["", "b"], "kind" => "x", "labels" => ["y"], "score" => "2", "size" => "m"})

    assert attribute(html, "input[type=checkbox][name='post[tags][]']", "value") == ["a", "b"]
    assert attribute(html, "input[type=checkbox][name='post[tags][]'][checked]", "value") == ["b"]
    assert attribute(html, "input[type=hidden][name='post[tags][]']", "value") == [""]

    assert attribute(html, "select#post-form_kind option", "value") == ["", "x"]
    assert attribute(html, "select#post-form_kind option[selected]", "value") == ["x"]
    assert LazyHTML.text(html) =~ "Pick one"

    assert attribute(html, "select#post-form_labels", "name") == ["post[labels][]"]
    assert attribute(html, "select#post-form_labels option[selected]", "value") == ["y"]

    assert attribute(html, "input[type=radio][name='post[score]']", "value") == ["1", "2", "3"]
    assert attribute(html, "input[type=radio][name='post[score]'][checked]", "value") == ["2"]
    assert attribute(html, "input[type=radio][name='post[size]'][checked]", "value") == ["m"]
  end

  test "a boolean is one checkbox, and a text input keeps its type and placeholder" do
    html = render_form(%{"agree" => "true", "email" => "nope"})

    assert attribute(html, "input#post-form_agree[checked]", "value") == ["true"]
    assert attribute(html, "input#post-form_email", "type") == ["email"]
    assert attribute(html, "input#post-form_email", "placeholder") == ["you@example.com"]
    assert attribute(html, "input#post-form_email", "value") == ["nope"]
    assert LazyHTML.text(html) =~ "has invalid format"
  end

  test "the submit button is the app's button, disabled while saving" do
    html = render_form(%{})

    assert attribute(html, "button[type=submit]", "class") == ["btn btn-primary btn-soft"]
    assert LazyHTML.text(LazyHTML.query(html, "button[type=submit]")) =~ "Post"

    html = render_form(%{}, disabled: true)

    assert attribute(html, "button[type=submit][disabled]", "class") != []
    assert LazyHTML.text(LazyHTML.query(html, "button[type=submit]")) =~ "Saving..."
  end
end

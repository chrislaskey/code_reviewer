defmodule Mix.Tasks.App.DynamicForms do
  @shortdoc "Writes @universal_dynamic_form form definitions as .json files"

  @moduledoc """
  Writes a `.dynamic_form.json` file next to every function marked
  `@universal_dynamic_form true`, so the desktop and mobile apps can
  render and validate the same form without the BEAM. The web app never
  reads these files: it calls the function.

      @universal_dynamic_form true
      def post_form do
        %{
          "id" => "post-form",
          "elements" => [
            %{"type" => "comment", "name" => "body", "title" => "Write a post", "isRequired" => true}
          ]
        }
      end

  writes `post_form.dynamic_form.json` in the module's folder. A marked
  function is public, takes no arguments and returns JSON data that
  `dynamic_form` reads. What only the server knows at render time, like
  the user's communities as choices, is filled in after calling it, by
  each app.

  ## Usage

      mix app.dynamic_forms          # write every file that is missing or out of date
      mix app.dynamic_forms --check  # report files out of date with their form

  `test/dynamic_form_json/sync_test.exs` runs the same check as `--check`,
  so `mix test` fails on a file left out of date.
  """

  use Mix.Task

  @switches [check: :boolean]

  @impl Mix.Task
  def run(argv) do
    {opts, _args} = OptionParser.parse!(argv, strict: @switches)
    Mix.Task.run("compile")

    {forms, problems} = DynamicFormJson.forms(Mix.Project.config()[:app])

    results =
      if opts[:check],
        do: DynamicFormJson.check(forms) ++ DynamicFormJson.orphans(forms),
        else: DynamicFormJson.build(forms) ++ DynamicFormJson.orphans(forms)

    {problems, done} = Enum.split_with(problems ++ results, &DynamicFormJson.problem?/1)

    for result <- done, not match?({:unchanged, _}, result), do: Mix.shell().info(DynamicFormJson.describe(result))
    for problem <- problems, do: Mix.shell().error(DynamicFormJson.describe(problem))

    if problems != [] do
      Mix.raise("mix app.dynamic_forms found #{length(problems)} problem(s)")
    end
  end
end

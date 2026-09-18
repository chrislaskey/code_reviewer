defmodule Mix.Tasks.CodeReviewer.CompareParsers do
  @shortdoc "Check the regex outline parser against the AST parser on a real codebase"

  @moduledoc """
  Runs both outline parsers over every `.ex`/`.exs` file under the given
  paths and reports files where they disagree on
  `{module, kind, name, arity, line_start, line_end, clause_count}`.

      mix code_reviewer.compare_parsers ~/code/form_flow
      mix code_reviewer.compare_parsers ~/code/form_flow/lib ~/code/other/lib

  Exits non-zero when any file differs, so it can gate CI against a corpus.
  """

  use Mix.Task

  alias CodeReviewer.Outline

  @impl Mix.Task
  def run(argv) do
    roots = if argv == [], do: ["."], else: argv

    files =
      roots
      |> Enum.flat_map(fn root ->
        root = Path.expand(root)
        if File.dir?(root), do: Path.wildcard(Path.join(root, "**/*.{ex,exs}")), else: [root]
      end)
      |> Enum.reject(&String.contains?(&1, ["/deps/", "/_build/", "/node_modules/"]))

    results = Enum.map(files, &compare/1)
    identical = Enum.count(results, &(elem(&1, 1) == :identical))
    unparsed = Enum.count(results, &(elem(&1, 1) == :unparsed))
    differing = Enum.filter(results, &match?({_, {:differs, _, _}}, &1))

    Mix.shell().info(
      "files #{length(files)}  identical #{identical}  differing #{length(differing)}  ast parse failures #{unparsed}"
    )

    for {path, {:differs, ast_only, regex_only}} <- differing do
      Mix.shell().info("\n== #{path}")
      for row <- Enum.take(ast_only, 8), do: Mix.shell().info("  AST only:   #{inspect(row)}")
      for row <- Enum.take(regex_only, 8), do: Mix.shell().info("  REGEX only: #{inspect(row)}")
    end

    if differing != [], do: exit({:shutdown, 1})
  end

  defp compare(path) do
    src = File.read!(path)

    case Outline.AST.parse(src) do
      {:error, _} ->
        {path, :unparsed}

      {:ok, ast_modules} ->
        a = ast_modules |> signature() |> Enum.sort()
        r = src |> Outline.Regex.parse() |> signature() |> Enum.sort()
        if a == r, do: {path, :identical}, else: {path, {:differs, a -- r, r -- a}}
    end
  end

  defp signature(modules) do
    for m <- modules, f <- m.functions do
      {m.name, f.kind, f.name, f.arity, f.line_start, f.line_end, length(f.clauses)}
    end
  end
end

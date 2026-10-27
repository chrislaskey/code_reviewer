//! Elixir extraction on top of the tree-sitter-elixir grammar.
//!
//! Source is parsed as text, never compiled, so macro-generated functions
//! and `use`-injected imports are invisible. Calls are matched by name and
//! arity only.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tree_sitter::{Node, Parser};

use super::error::AnalysisError;
use super::language::LanguageAnalyzer;
use super::model::{
    FileSymbols, FunctionDefinition, FunctionId, FunctionKind, ImportDirective, LineRange, RawCall,
    RawCallTarget,
};

pub struct ElixirAnalyzer;

impl LanguageAnalyzer for ElixirAnalyzer {
    fn language_name(&self) -> &'static str {
        "elixir"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["ex", "exs"]
    }

    fn parse_file(&self, file_path: &str, source: &str) -> Result<FileSymbols, AnalysisError> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_elixir::LANGUAGE.into())
            .map_err(|error| AnalysisError::ParserSetupFailed {
                language: self.language_name().to_string(),
                message: error.to_string(),
            })?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| AnalysisError::ParseFailed {
                path: file_path.to_string(),
            })?;

        let mut collector = Collector::new(file_path, source);
        collector.visit_module_items(tree.root_node(), &mut Scope::default());
        Ok(collector.finish(tree.root_node().has_error()))
    }
}

/// Lexical state while walking: current module, aliases and imports.
#[derive(Debug, Clone, Default)]
struct Scope {
    module: String,
    aliases: HashMap<String, String>,
    imports: Arc<Vec<ImportDirective>>,
}

impl Scope {
    /// Expands `Short.Rest` through aliases and `__MODULE__`. Returns `None`
    /// for anything that is not a plain module reference.
    fn resolve_module(&self, written: &str) -> Option<String> {
        let mut segments = written.split('.');
        let first = segments.next()?;
        let rest: Vec<&str> = segments.collect();
        if !rest.iter().all(|segment| is_alias_segment(segment)) {
            return None;
        }

        let base = if first == "__MODULE__" {
            if self.module.is_empty() {
                return None;
            }
            self.module.clone()
        } else if is_alias_segment(first) {
            self.aliases
                .get(first)
                .cloned()
                .unwrap_or_else(|| first.to_string())
        } else {
            return None;
        };

        Some(
            std::iter::once(base.as_str())
                .chain(rest)
                .collect::<Vec<_>>()
                .join("."),
        )
    }

    fn add_alias(&mut self, short_name: String, full_name: String) {
        self.aliases.insert(short_name, full_name);
    }

    fn add_import(&mut self, directive: ImportDirective) {
        Arc::make_mut(&mut self.imports).push(directive);
    }
}

fn is_alias_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    characters.next().is_some_and(|c| c.is_ascii_uppercase())
        && characters.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `"as: "` -> `"as"`.
fn keyword_name(written: &str) -> &str {
    written.trim().trim_end_matches(':')
}

fn last_segment(module: &str) -> &str {
    module.rsplit('.').next().unwrap_or(module)
}

/// One `def`-style clause, merged into a function by `finish`.
struct Clause {
    id: FunctionId,
    kind: FunctionKind,
    default_argument_count: usize,
    range: LineRange,
    start_line: usize,
}

struct Collector<'a> {
    file_path: &'a str,
    source: &'a [u8],
    clauses: Vec<Clause>,
    calls: Vec<RawCall>,
}

impl<'a> Collector<'a> {
    fn new(file_path: &'a str, source: &'a str) -> Self {
        Self {
            file_path,
            source: source.as_bytes(),
            clauses: Vec::new(),
            calls: Vec::new(),
        }
    }

    fn text(&self, node: Node) -> &'a str {
        node.utf8_text(self.source).unwrap_or("")
    }

    fn line_of(node: Node) -> usize {
        node.start_position().row + 1
    }

    fn finish(self, had_syntax_errors: bool) -> FileSymbols {
        let mut merged: BTreeMap<FunctionId, FunctionDefinition> = BTreeMap::new();
        for clause in self.clauses {
            merged
                .entry(clause.id.clone())
                .and_modify(|function| {
                    function.start_line = function.start_line.min(clause.start_line);
                    function.end_line = function.end_line.max(clause.range.end_line);
                    function.default_argument_count = function
                        .default_argument_count
                        .max(clause.default_argument_count);
                    function.clause_ranges.push(clause.range);
                })
                .or_insert_with(|| FunctionDefinition {
                    visibility: clause.kind.visibility(),
                    kind: clause.kind,
                    file_path: self.file_path.to_string(),
                    default_argument_count: clause.default_argument_count,
                    start_line: clause.start_line,
                    end_line: clause.range.end_line,
                    clause_ranges: vec![clause.range],
                    id: clause.id,
                });
        }

        let mut functions: Vec<FunctionDefinition> = merged.into_values().collect();
        functions.sort_by_key(|function| function.start_line);

        FileSymbols {
            file_path: self.file_path.to_string(),
            functions,
            calls: self.calls,
            had_syntax_errors,
        }
    }

    // -- module level -------------------------------------------------------

    /// Walks the statements of a file or a `do ... end` block.
    fn visit_module_items(&mut self, container: Node, scope: &mut Scope) {
        let mut annotation_start: Option<usize> = None;
        let mut cursor = container.walk();
        for item in container.named_children(&mut cursor) {
            match item.kind() {
                "unary_operator" => {
                    if self.is_leading_annotation(item) {
                        annotation_start.get_or_insert(Self::line_of(item));
                    }
                }
                "call" => {
                    self.visit_module_call(item, scope, annotation_start.take());
                }
                _ => {}
            }
        }
    }

    /// True for `@doc`, `@spec`, `@impl` and `@deprecated`.
    fn is_leading_annotation(&self, node: Node) -> bool {
        let is_attribute = node
            .child_by_field_name("operator")
            .is_some_and(|operator| self.text(operator) == "@");
        let name = node
            .child_by_field_name("operand")
            .filter(|operand| operand.kind() == "call")
            .and_then(|operand| operand.child_by_field_name("target"))
            .map(|target| self.text(target));
        is_attribute && matches!(name, Some("doc" | "spec" | "impl" | "deprecated"))
    }

    fn visit_module_call(
        &mut self,
        call: Node,
        scope: &mut Scope,
        annotation_start: Option<usize>,
    ) {
        let Some(target) = call.child_by_field_name("target") else {
            return;
        };
        if target.kind() != "identifier" {
            return;
        }

        match self.text(target) {
            "alias" => self.apply_alias(call, scope),
            "import" => self.apply_import(call, scope),
            "defmodule" | "defprotocol" => self.visit_nested_module(call, scope),
            "defimpl" => self.visit_implementation(call, scope),
            "def" => self.record_definition(call, FunctionKind::Def, scope, annotation_start),
            "defp" => self.record_definition(call, FunctionKind::Defp, scope, annotation_start),
            "defmacro" => {
                self.record_definition(call, FunctionKind::Defmacro, scope, annotation_start)
            }
            "defmacrop" => {
                self.record_definition(call, FunctionKind::Defmacrop, scope, annotation_start)
            }
            "defguard" => {
                self.record_definition(call, FunctionKind::Defguard, scope, annotation_start)
            }
            "defguardp" => {
                self.record_definition(call, FunctionKind::Defguardp, scope, annotation_start)
            }
            "defdelegate" => {
                self.record_delegate(call, scope, annotation_start);
            }
            "test" => self.record_test(call, scope),
            // Wrappers that hold ordinary definitions.
            "describe" | "if" | "unless" => {
                if let Some(block) = find_child(call, "do_block") {
                    self.visit_module_items(block, scope);
                }
            }
            _ => {}
        }
    }

    fn visit_nested_module(&mut self, call: Node, scope: &mut Scope) {
        let Some(name_node) = first_argument(call) else {
            return;
        };
        let written = self.text(name_node);
        let Some(block) = find_child(call, "do_block") else {
            return;
        };

        let uses_current_module = written.starts_with("__MODULE__");
        let full_name = match (uses_current_module, scope.module.is_empty()) {
            (true, _) | (false, true) => scope.resolve_module(written),
            (false, false) => Some(format!("{}.{}", scope.module, written)),
        };
        let Some(full_name) = full_name else {
            return;
        };

        // A nested `defmodule B.C` inside `A` also aliases `B` to `A.B`.
        if !uses_current_module && !scope.module.is_empty() {
            let first = written.split('.').next().unwrap_or(written);
            scope.add_alias(first.to_string(), format!("{}.{}", scope.module, first));
        }

        let mut inner = scope.clone();
        inner.module = full_name;
        self.visit_module_items(block, &mut inner);
    }

    /// `defimpl Protocol, for: Type` becomes the module `Protocol.Type`.
    fn visit_implementation(&mut self, call: Node, scope: &mut Scope) {
        let Some(protocol_node) = first_argument(call) else {
            return;
        };
        let Some(block) = find_child(call, "do_block") else {
            return;
        };
        let protocol = scope.resolve_module(self.text(protocol_node));
        let target = self
            .keyword_value(call, "for")
            .and_then(|node| scope.resolve_module(self.text(node)));
        let (Some(protocol), Some(target)) = (protocol, target) else {
            return;
        };

        let mut inner = scope.clone();
        inner.module = format!("{protocol}.{target}");
        self.visit_module_items(block, &mut inner);
    }

    // -- directives ---------------------------------------------------------

    fn apply_alias(&mut self, call: Node, scope: &mut Scope) {
        let Some(argument) = first_argument(call) else {
            return;
        };

        // alias Foo.{Bar, Baz.Qux}
        let grouped = argument
            .child_by_field_name("right")
            .filter(|right| right.kind() == "tuple")
            .zip(argument.child_by_field_name("left"));
        if let Some((tuple, left)) = grouped {
            let Some(prefix) = scope.resolve_module(self.text(left)) else {
                return;
            };
            let mut cursor = tuple.walk();
            let members: Vec<&str> = tuple
                .named_children(&mut cursor)
                .map(|member| self.text(member))
                .collect();
            for member in members {
                scope.add_alias(
                    last_segment(member).to_string(),
                    format!("{prefix}.{member}"),
                );
            }
            return;
        }

        let Some(full_name) = scope.resolve_module(self.text(argument)) else {
            return;
        };
        let short_name = self
            .keyword_value(call, "as")
            .map(|node| self.text(node).to_string())
            .unwrap_or_else(|| last_segment(&full_name).to_string());
        scope.add_alias(short_name, full_name);
    }

    fn apply_import(&mut self, call: Node, scope: &mut Scope) {
        let Some(argument) = first_argument(call) else {
            return;
        };
        let Some(module) = scope.resolve_module(self.text(argument)) else {
            return;
        };
        let only = self
            .keyword_value(call, "only")
            .filter(|value| value.kind() == "list")
            .map(|list| self.read_name_arity_pairs(list));
        scope.add_import(ImportDirective { module, only });
    }

    /// Reads `[name: arity, ...]` from a list node.
    fn read_name_arity_pairs(&self, list: Node) -> Vec<(String, usize)> {
        let mut pairs = Vec::new();
        let mut cursor = list.walk();
        for keywords in list.named_children(&mut cursor) {
            let mut inner = keywords.walk();
            for pair in keywords.named_children(&mut inner) {
                let name = pair.child_by_field_name("key").map(|key| self.text(key));
                let arity = pair
                    .child_by_field_name("value")
                    .and_then(|value| self.text(value).parse::<usize>().ok());
                if let (Some(name), Some(arity)) = (name, arity) {
                    pairs.push((keyword_name(name).to_string(), arity));
                }
            }
        }
        pairs
    }

    /// Finds `key:` in a call's trailing keyword list and returns its value node.
    fn keyword_value<'t>(&self, call: Node<'t>, key: &str) -> Option<Node<'t>> {
        let arguments = find_child(call, "arguments")?;
        let mut cursor = arguments.walk();
        let keywords = arguments
            .named_children(&mut cursor)
            .find(|child| child.kind() == "keywords")?;
        let mut inner = keywords.walk();
        let value = keywords.named_children(&mut inner).find_map(|pair| {
            let pair_key = pair.child_by_field_name("key")?;
            let matches = keyword_name(self.text(pair_key)) == key;
            matches.then(|| pair.child_by_field_name("value")).flatten()
        });
        value
    }

    // -- definitions --------------------------------------------------------

    fn record_definition(
        &mut self,
        call: Node,
        kind: FunctionKind,
        scope: &Scope,
        annotation_start: Option<usize>,
    ) {
        let Some(arguments) = find_child(call, "arguments") else {
            return;
        };
        let Some(head) = arguments.named_child(0) else {
            return;
        };
        let Some((name, parameters)) = self.read_function_head(head) else {
            return;
        };
        if scope.module.is_empty() {
            return;
        }

        let default_argument_count = parameters
            .iter()
            .filter(|parameter| self.is_default_argument(**parameter))
            .count();
        let id = FunctionId::new(&scope.module, name, parameters.len());
        self.push_clause(id, kind, default_argument_count, call, annotation_start);

        // Everything after the head is body: `do:` keywords and `do ... end`.
        let mut function_scope = scope.clone();
        let mut cursor = arguments.walk();
        let body_arguments: Vec<Node> = arguments.named_children(&mut cursor).skip(1).collect();
        let caller = self.clauses.last().map(|clause| clause.id.clone());
        let Some(caller) = caller else {
            return;
        };
        for argument in body_arguments {
            self.walk_calls(argument, 0, &mut function_scope, &caller);
        }
        if let Some(block) = find_child(call, "do_block") {
            self.walk_calls(block, 0, &mut function_scope, &caller);
        }
    }

    /// Returns the function name and its parameter nodes from a `def` head.
    fn read_function_head<'t>(&self, head: Node<'t>) -> Option<(String, Vec<Node<'t>>)> {
        let head = if head.kind() == "binary_operator"
            && head
                .child_by_field_name("operator")
                .is_some_and(|operator| self.text(operator) == "when")
        {
            head.child_by_field_name("left")?
        } else {
            head
        };

        match head.kind() {
            "identifier" => Some((self.text(head).to_string(), Vec::new())),
            "call" => {
                let target = head.child_by_field_name("target")?;
                if target.kind() != "identifier" {
                    return None;
                }
                let parameters = find_child(head, "arguments")
                    .map(|arguments| {
                        let mut cursor = arguments.walk();
                        arguments.named_children(&mut cursor).collect()
                    })
                    .unwrap_or_default();
                Some((self.text(target).to_string(), parameters))
            }
            _ => None,
        }
    }

    fn is_default_argument(&self, parameter: Node) -> bool {
        parameter.kind() == "binary_operator"
            && parameter
                .child_by_field_name("operator")
                .is_some_and(|operator| self.text(operator) == "\\\\")
    }

    fn push_clause(
        &mut self,
        id: FunctionId,
        kind: FunctionKind,
        default_argument_count: usize,
        node: Node,
        annotation_start: Option<usize>,
    ) {
        let start_line = Self::line_of(node);
        let end_line = node.end_position().row + 1;
        self.clauses.push(Clause {
            id,
            kind,
            default_argument_count,
            range: LineRange {
                start_line,
                end_line,
            },
            start_line: annotation_start.unwrap_or(start_line).min(start_line),
        });
    }

    /// `defdelegate name(args), to: Module, as: other` is a function that
    /// calls `Module.other/arity`.
    fn record_delegate(&mut self, call: Node, scope: &Scope, annotation_start: Option<usize>) {
        let Some(head) = first_argument(call) else {
            return;
        };
        let Some((name, parameters)) = self.read_function_head(head) else {
            return;
        };
        if scope.module.is_empty() {
            return;
        }

        let arity = parameters.len();
        let id = FunctionId::new(&scope.module, &name, arity);
        self.push_clause(
            id.clone(),
            FunctionKind::Defdelegate,
            0,
            call,
            annotation_start,
        );

        let delegate_module = self
            .keyword_value(call, "to")
            .and_then(|node| scope.resolve_module(self.text(node)));
        let delegate_name = self
            .keyword_value(call, "as")
            .map(|node| self.text(node).trim_start_matches(':').to_string())
            .unwrap_or(name);
        if let Some(module) = delegate_module {
            self.calls.push(RawCall {
                caller: id,
                target: RawCallTarget::Remote {
                    module,
                    name: delegate_name,
                    arity,
                },
                line: Self::line_of(call),
            });
        }
    }

    /// ExUnit `test "description" do ... end`.
    fn record_test(&mut self, call: Node, scope: &Scope) {
        let Some(arguments) = find_child(call, "arguments") else {
            return;
        };
        let Some(description) = arguments.named_child(0).filter(|n| n.kind() == "string") else {
            return;
        };
        if scope.module.is_empty() {
            return;
        }

        let name = format!("test {}", self.text(description).trim_matches('"'));
        let id = FunctionId::new(&scope.module, name, 0);
        self.push_clause(id.clone(), FunctionKind::Test, 0, call, None);

        let mut function_scope = scope.clone();
        let mut cursor = arguments.walk();
        let rest: Vec<Node> = arguments.named_children(&mut cursor).skip(1).collect();
        for argument in rest {
            self.walk_calls(argument, 0, &mut function_scope, &id);
        }
        if let Some(block) = find_child(call, "do_block") {
            self.walk_calls(block, 0, &mut function_scope, &id);
        }
    }

    // -- calls inside function bodies ----------------------------------------

    /// Records every call under `node`. `piped_arguments` is 1 for the right
    /// side of `|>`, which receives one more argument than it spells out.
    fn walk_calls(
        &mut self,
        node: Node,
        piped_arguments: usize,
        scope: &mut Scope,
        caller: &FunctionId,
    ) {
        match node.kind() {
            "call" => self.walk_call(node, piped_arguments, scope, caller),
            "binary_operator" if self.operator_of(node) == Some("|>") => {
                if let Some(left) = node.child_by_field_name("left") {
                    self.walk_calls(left, 0, scope, caller);
                }
                if let Some(right) = node.child_by_field_name("right") {
                    self.walk_calls(right, 1, scope, caller);
                }
            }
            "unary_operator" if self.record_capture(node, scope, caller) => {}
            _ => self.walk_children(node, scope, caller),
        }
    }

    fn walk_children(&mut self, node: Node, scope: &mut Scope, caller: &FunctionId) {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.named_children(&mut cursor).collect();
        for child in children {
            self.walk_calls(child, 0, scope, caller);
        }
    }

    fn operator_of(&self, node: Node) -> Option<&'a str> {
        node.child_by_field_name("operator")
            .map(|operator| self.text(operator))
    }

    fn walk_call(
        &mut self,
        call: Node,
        piped_arguments: usize,
        scope: &mut Scope,
        caller: &FunctionId,
    ) {
        let Some(target) = call.child_by_field_name("target") else {
            return self.walk_children(call, scope, caller);
        };

        let arity = self.count_arguments(call) + piped_arguments;
        match target.kind() {
            "identifier" => match self.text(target) {
                "alias" => return self.apply_alias(call, scope),
                "import" => return self.apply_import(call, scope),
                "require" | "use" => return,
                name => self.push_local_call(name, arity, target, scope, caller),
            },
            "dot" => self.push_remote_call(target, arity, scope, caller),
            _ => {}
        }
        self.walk_children(call, scope, caller);
    }

    fn count_arguments(&self, call: Node) -> usize {
        let written =
            find_child(call, "arguments").map_or(0, |arguments| arguments.named_child_count());
        let block = usize::from(find_child(call, "do_block").is_some());
        written + block
    }

    fn push_local_call(
        &mut self,
        name: &str,
        arity: usize,
        target: Node,
        scope: &Scope,
        caller: &FunctionId,
    ) {
        self.calls.push(RawCall {
            caller: caller.clone(),
            target: RawCallTarget::Local {
                module: scope.module.clone(),
                name: name.to_string(),
                arity,
                imports: Arc::clone(&scope.imports),
            },
            line: Self::line_of(target),
        });
    }

    /// `Module.function` where `Module` is an alias or `__MODULE__`.
    fn push_remote_call(&mut self, dot: Node, arity: usize, scope: &Scope, caller: &FunctionId) {
        let (Some(left), Some(right)) = (
            dot.child_by_field_name("left"),
            dot.child_by_field_name("right"),
        ) else {
            return;
        };
        if right.kind() != "identifier" {
            return;
        }
        let Some(module) = scope.resolve_module(self.text(left)) else {
            return;
        };
        self.calls.push(RawCall {
            caller: caller.clone(),
            target: RawCallTarget::Remote {
                module,
                name: self.text(right).to_string(),
                arity,
            },
            line: Self::line_of(right),
        });
    }

    /// `&Module.function/2` and `&function/2`. Returns false when the node is
    /// some other `&` expression, which is walked normally.
    fn record_capture(&mut self, node: Node, scope: &Scope, caller: &FunctionId) -> bool {
        if self.operator_of(node) != Some("&") {
            return false;
        }
        let Some(operand) = node
            .child_by_field_name("operand")
            .filter(|operand| operand.kind() == "binary_operator")
            .filter(|operand| self.operator_of(*operand) == Some("/"))
        else {
            return false;
        };
        let arity = operand
            .child_by_field_name("right")
            .filter(|right| right.kind() == "integer")
            .and_then(|right| self.text(right).parse::<usize>().ok());
        let (Some(arity), Some(function)) = (arity, operand.child_by_field_name("left")) else {
            return false;
        };

        match function.kind() {
            "identifier" => {
                self.push_local_call(self.text(function), arity, function, scope, caller);
                true
            }
            "call" => match function.child_by_field_name("target") {
                Some(dot) if dot.kind() == "dot" => {
                    self.push_remote_call(dot, arity, scope, caller);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }
}

fn find_child<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    let child = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind);
    child
}

fn first_argument(call: Node) -> Option<Node> {
    find_child(call, "arguments")?.named_child(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> FileSymbols {
        ElixirAnalyzer
            .parse_file("lib/example.ex", source)
            .expect("parses")
    }

    fn remote(module: &str, name: &str, arity: usize) -> RawCallTarget {
        RawCallTarget::Remote {
            module: module.to_string(),
            name: name.to_string(),
            arity,
        }
    }

    fn call_targets(symbols: &FileSymbols, function: &str) -> Vec<RawCallTarget> {
        symbols
            .calls
            .iter()
            .filter(|call| call.caller.name == function)
            .map(|call| call.target.clone())
            .collect()
    }

    #[test]
    fn test_functions_merge_clauses_and_include_leading_annotations() {
        let symbols = parse(
            r#"defmodule A do
  @doc "Adds"
  @spec add(integer, integer) :: integer
  def add(1, b), do: b
  def add(a, b) do
    a + b
  end

  defp hidden(x), do: x
end
"#,
        );

        let add = &symbols.functions[0];
        assert_eq!(add.id, FunctionId::new("A", "add", 2));
        assert_eq!((add.start_line, add.end_line), (2, 7));
        assert_eq!(add.clause_ranges.len(), 2);
        assert_eq!(
            symbols.functions[1].visibility,
            crate::analysis::model::Visibility::Private
        );
    }

    #[test]
    fn test_default_arguments_widen_accepted_arity() {
        let symbols = parse("defmodule A do\n  def go(a, b \\\\ 1, c \\\\ 2), do: a\nend\n");
        let go = &symbols.functions[0];
        assert_eq!(go.id.arity, 3);
        assert!(go.accepts_arity(1) && go.accepts_arity(3));
        assert!(!go.accepts_arity(0) && !go.accepts_arity(4));
    }

    #[test]
    fn test_nested_modules_get_qualified_names() {
        let symbols = parse(
            "defmodule A do\n  defmodule B do\n    def one, do: 1\n  end\n  def two, do: B.one()\nend\n",
        );
        let ids: Vec<String> = symbols.functions.iter().map(|f| f.id.to_string()).collect();
        assert_eq!(ids, vec!["A.B.one/0", "A.two/0"]);
        assert_eq!(call_targets(&symbols, "two"), vec![remote("A.B", "one", 0)]);
    }

    #[test]
    fn test_remote_calls_resolve_aliases() {
        let symbols = parse(
            r#"defmodule A do
  alias Foo.{Bar, Baz.Qux}
  alias Other.Thing, as: T
  alias Plain.Name

  def go(x) do
    Bar.one(x)
    Qux.two()
    T.three(x, x)
    Name.four
    __MODULE__.five(x)
  end
end
"#,
        );
        assert_eq!(
            call_targets(&symbols, "go"),
            vec![
                remote("Foo.Bar", "one", 1),
                remote("Foo.Baz.Qux", "two", 0),
                remote("Other.Thing", "three", 2),
                remote("Plain.Name", "four", 0),
                remote("A", "five", 1),
            ]
        );
    }

    #[test]
    fn test_pipes_add_one_to_the_arity_of_the_piped_call() {
        let symbols = parse(
            "defmodule A do\n  def go(x) do\n    x |> Mod.one(1) |> two() |> Mod.three()\n  end\nend\n",
        );
        let targets = call_targets(&symbols, "go");
        assert!(targets.contains(&remote("Mod", "one", 2)));
        assert!(targets.contains(&remote("Mod", "three", 1)));
        assert!(targets.iter().any(|t| matches!(
            t,
            RawCallTarget::Local { name, arity: 1, .. } if name == "two"
        )));
    }

    #[test]
    fn test_captures_are_calls_with_the_written_arity() {
        let symbols = parse(
            "defmodule A do\n  def go(list) do\n    Enum.map(list, &Mod.convert/1) ++ Enum.map(list, &local/2)\n  end\nend\n",
        );
        let targets = call_targets(&symbols, "go");
        assert!(targets.contains(&remote("Mod", "convert", 1)));
        assert!(targets.iter().any(|t| matches!(
            t,
            RawCallTarget::Local { name, arity: 2, .. } if name == "local"
        )));
    }

    #[test]
    fn test_defdelegate_calls_the_target_module() {
        let symbols = parse("defmodule A do\n  defdelegate get(id), to: Store, as: :fetch\nend\n");
        assert_eq!(symbols.functions[0].kind, FunctionKind::Defdelegate);
        assert_eq!(
            call_targets(&symbols, "get"),
            vec![remote("Store", "fetch", 1)]
        );
    }

    #[test]
    fn test_exunit_tests_are_functions_inside_describe() {
        let symbols = parse(
            "defmodule ATest do\n  describe \"go\" do\n    test \"works\" do\n      A.go(1)\n    end\n  end\nend\n",
        );
        assert_eq!(symbols.functions[0].id.name, "test works");
        assert_eq!(symbols.functions[0].kind, FunctionKind::Test);
        assert_eq!(
            call_targets(&symbols, "test works"),
            vec![remote("A", "go", 1)]
        );
    }

    #[test]
    fn test_imports_record_only_lists() {
        let symbols =
            parse("defmodule A do\n  import Helpers, only: [one: 1]\n  def go, do: one(1)\nend\n");
        let call = &symbols.calls[0];
        let RawCallTarget::Local { imports, .. } = &call.target else {
            panic!("expected a local call");
        };
        assert_eq!(imports[0].module, "Helpers");
        assert!(imports[0].brings_into_scope("one", 1));
        assert!(!imports[0].brings_into_scope("two", 1));
    }

    #[test]
    fn test_syntax_errors_are_reported_but_parsing_continues() {
        let symbols = parse("defmodule A do\n  def ok, do: 1\n  def broken(\nend\n");
        assert!(symbols.had_syntax_errors);
    }
}

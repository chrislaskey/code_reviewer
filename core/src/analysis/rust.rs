//! Rust extraction on top of the tree-sitter-rust grammar.
//!
//! Source is parsed as text, never type-checked. Paths are followed through
//! `use` declarations and the file's own items (`Orders::place`, `Self::go`,
//! `super::helper`), and `self.method()` is looked up on the type being
//! implemented. A method call on any other value (`order.total()`) is matched
//! by name alone, and only when the project defines exactly one function of
//! that name and arity. Calls inside macro arguments (`assert_eq!`, `format!`)
//! are found by parsing the arguments again as expressions. Macro-generated
//! functions are invisible.
//!
//! A function's module is its file's module path (`core/src/git.rs` is
//! `core::git`), plus the type for methods (`core::git::Repo`). Arity leaves
//! out `self`, so it matches the arguments a call writes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use tree_sitter::{Node, Parser};

use super::error::AnalysisError;
use super::language::LanguageAnalyzer;
use super::model::{
    FileSymbols, FunctionDefinition, FunctionId, FunctionKind, ImportDirective, LineRange, RawCall,
    RawCallTarget, Visibility,
};

pub struct RustAnalyzer;

impl LanguageAnalyzer for RustAnalyzer {
    fn language_name(&self) -> &'static str {
        "rust"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn parse_file(&self, file_path: &str, source: &str) -> Result<FileSymbols, AnalysisError> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|error| AnalysisError::ParserSetupFailed {
                language: self.language_name().to_string(),
                message: error.to_string(),
            })?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| AnalysisError::ParseFailed {
                path: file_path.to_string(),
            })?;

        let mut collector = Collector::new(file_path, source, parser);
        let mut scope = Scope::for_file(file_path);
        collector.visit_module_items(tree.root_node(), &mut scope, None);
        Ok(collector.finish(tree.root_node().has_error()))
    }
}

/// Methods of the standard library that a project type may also define. A
/// call on a value of unknown type is not matched to a project function of
/// one of these names, since it is far more likely a `Vec`, `Option`, `str`
/// or `HashMap` method.
const COMMON_STD_METHOD_NAMES: &[&str] = &[
    "all",
    "and_then",
    "any",
    "as_bytes",
    "as_deref",
    "as_mut",
    "as_ref",
    "as_slice",
    "as_str",
    "borrow",
    "borrow_mut",
    "bytes",
    "chain",
    "chars",
    "clear",
    "clone",
    "cloned",
    "cmp",
    "collect",
    "contains",
    "contains_key",
    "copied",
    "count",
    "dedup",
    "default",
    "drain",
    "drop",
    "ends_with",
    "entry",
    "enumerate",
    "eq",
    "err",
    "expect",
    "extend",
    "filter",
    "filter_map",
    "find",
    "find_map",
    "first",
    "flat_map",
    "flatten",
    "flush",
    "fmt",
    "fold",
    "for_each",
    "from",
    "get",
    "get_mut",
    "get_or_insert",
    "get_or_insert_with",
    "hash",
    "insert",
    "into",
    "into_iter",
    "is_empty",
    "is_err",
    "is_none",
    "is_ok",
    "is_some",
    "is_some_and",
    "iter",
    "iter_mut",
    "join",
    "keys",
    "last",
    "len",
    "lines",
    "lock",
    "map",
    "map_err",
    "map_or",
    "matches",
    "max",
    "min",
    "next",
    "ok",
    "ok_or",
    "ok_or_else",
    "or_else",
    "parse",
    "pop",
    "position",
    "push",
    "read",
    "recv",
    "remove",
    "replace",
    "retain",
    "rev",
    "send",
    "skip",
    "sort",
    "sort_by",
    "sort_by_key",
    "split",
    "starts_with",
    "sum",
    "take",
    "then",
    "then_some",
    "to_lowercase",
    "to_owned",
    "to_string",
    "to_uppercase",
    "to_vec",
    "trim",
    "unwrap",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "values",
    "write",
    "zip",
];

/// The crate and module a file holds: `core/src/analysis/rust.rs` is crate
/// `core`, module `core::analysis::rust`; `src/lib.rs` is crate `crate` and
/// its root module; `core/tests/analysis.rs` is `core::tests::analysis`.
fn module_path_of_file(file_path: &str) -> (String, String) {
    let without_extension = file_path.strip_suffix(".rs").unwrap_or(file_path);
    let components: Vec<&str> = without_extension.split('/').collect();
    let source_root = components
        .iter()
        .position(|component| matches!(*component, "src" | "tests" | "benches" | "examples"));

    let (crate_name, mut modules): (&str, Vec<&str>) = match source_root {
        Some(index) => {
            let crate_name = if index == 0 {
                "crate"
            } else {
                components[index - 1]
            };
            let first_module = if components[index] == "src" {
                index + 1
            } else {
                index
            };
            (crate_name, components[first_module..].to_vec())
        }
        None => {
            let (file, directories) = components.split_last().unwrap_or((&"", &[]));
            (directories.last().copied().unwrap_or("crate"), vec![*file])
        }
    };

    if modules.len() == 1 && matches!(modules[0], "lib" | "main") {
        modules.clear();
    }
    if modules.last() == Some(&"mod") {
        modules.pop();
    }

    let module = std::iter::once(crate_name)
        .chain(modules)
        .collect::<Vec<_>>()
        .join("::");
    (crate_name.to_string(), module)
}

/// Where a written path points.
enum PathTarget {
    /// One module inside the crate.
    Internal(String),
    /// Under one of the glob imports of the crate: `use super::*` makes `Foo::new`
    /// any of their `Foo`.
    Globbed(Vec<String>),
    /// Outside the crate (standard library, dependencies, other crates).
    External(String),
}

/// Lexical state while walking: current module, the type being implemented,
/// the names the module defines and its `use` declarations.
#[derive(Debug, Clone)]
struct Scope {
    crate_name: String,
    module: String,
    /// The module path of the `impl` or `trait` being walked, which `Self` names.
    self_type: Option<String>,
    /// Types, traits, functions and child modules this module defines.
    local_items: HashSet<String>,
    /// `use` aliases to full paths.
    uses: HashMap<String, String>,
    /// `use path::*` directives, searched for bare calls after the module itself.
    globs: Arc<Vec<ImportDirective>>,
}

impl Scope {
    fn for_file(file_path: &str) -> Self {
        let (crate_name, module) = module_path_of_file(file_path);
        Self {
            crate_name,
            module,
            self_type: None,
            local_items: HashSet::new(),
            uses: HashMap::new(),
            globs: Arc::new(Vec::new()),
        }
    }

    fn child_module(&self, name: &str) -> Self {
        Self {
            crate_name: self.crate_name.clone(),
            module: format!("{}::{name}", self.module),
            self_type: None,
            local_items: HashSet::new(),
            uses: HashMap::new(),
            globs: Arc::new(Vec::new()),
        }
    }

    fn parent_module(&self) -> Option<String> {
        self.module
            .rsplit_once("::")
            .map(|(parent, _)| parent.to_string())
    }

    fn is_internal(&self, path: &str) -> bool {
        path == self.crate_name || path.starts_with(&format!("{}::", self.crate_name))
    }

    fn internal_globs(&self) -> Vec<&str> {
        self.globs
            .iter()
            .map(|directive| directive.module.as_str())
            .filter(|module| self.is_internal(module))
            .collect()
    }

    /// Follows `crate`, `self`, `super`, `Self`, the module's own items and
    /// its `use` declarations. `None` for paths that cannot be followed.
    fn resolve_path(&self, segments: &[&str]) -> Option<PathTarget> {
        let (first, mut rest) = segments.split_first()?;
        let base = match *first {
            "crate" => self.crate_name.clone(),
            "self" => self.module.clone(),
            "super" => {
                let mut module = self.parent_module()?;
                while rest.first() == Some(&"super") {
                    module = module.rsplit_once("::")?.0.to_string();
                    rest = &rest[1..];
                }
                module
            }
            "Self" => self.self_type.clone()?,
            name if self.local_items.contains(name) => format!("{}::{name}", self.module),
            name => match self.uses.get(name) {
                Some(full) if self.is_internal(full) => full.clone(),
                Some(full) => return Some(PathTarget::External(join_path(full, rest))),
                None if starts_uppercase(name) && !self.internal_globs().is_empty() => {
                    let candidates = self
                        .internal_globs()
                        .into_iter()
                        .map(|glob| join_path(&format!("{glob}::{name}"), rest))
                        .collect();
                    return Some(PathTarget::Globbed(candidates));
                }
                None => return Some(PathTarget::External(join_path(name, rest))),
            },
        };
        Some(PathTarget::Internal(join_path(&base, rest)))
    }

    fn add_use(&mut self, alias: &str, segments: &[&str]) {
        let full = match self.resolve_path(segments) {
            Some(PathTarget::Internal(path) | PathTarget::External(path)) => path,
            Some(PathTarget::Globbed(_)) | None => return,
        };
        self.uses.insert(alias.to_string(), full);
    }

    fn add_glob(&mut self, segments: &[&str]) {
        let module = match self.resolve_path(segments) {
            Some(PathTarget::Internal(path) | PathTarget::External(path)) => path,
            Some(PathTarget::Globbed(_)) | None => return,
        };
        Arc::make_mut(&mut self.globs).push(ImportDirective { module, only: None });
    }
}

fn join_path(base: &str, rest: &[&str]) -> String {
    std::iter::once(base)
        .chain(rest.iter().copied())
        .collect::<Vec<_>>()
        .join("::")
}

fn starts_uppercase(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn starts_lowercase(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_lowercase() || c == '_')
}

/// One `fn`, merged by `finish` with any other of the same id (an `impl From<A>`
/// and an `impl From<B>` for one type both define `from`).
struct Clause {
    id: FunctionId,
    kind: FunctionKind,
    visibility: Visibility,
    range: LineRange,
    start_line: usize,
}

/// Attributes and doc comments directly above an item, kept until the item.
#[derive(Default)]
struct LeadingRun<'t> {
    start_line: Option<usize>,
    end_row: Option<usize>,
    attributes: Vec<Node<'t>>,
}

impl<'t> LeadingRun<'t> {
    fn is_contiguous_with(&self, node: Node) -> bool {
        self.end_row
            .is_none_or(|end_row| node.start_position().row <= end_row + 1)
    }

    fn extend(&mut self, node: Node<'t>) {
        if !self.is_contiguous_with(node) {
            *self = Self::default();
        }
        self.start_line.get_or_insert(node.start_position().row + 1);
        self.end_row = Some(node.end_position().row);
        if node.kind() == "attribute_item" {
            self.attributes.push(node);
        }
    }

    fn take_for(&mut self, item: Node) -> (Option<usize>, Vec<Node<'t>>) {
        let run = std::mem::take(self);
        if run.is_contiguous_with(item) {
            (run.start_line, run.attributes)
        } else {
            (None, Vec::new())
        }
    }
}

struct Collector<'a> {
    file_path: &'a str,
    source: &'a [u8],
    parser: Parser,
    clauses: Vec<Clause>,
    calls: Vec<RawCall>,
}

impl<'a> Collector<'a> {
    fn new(file_path: &'a str, source: &'a str, parser: Parser) -> Self {
        Self {
            file_path,
            source: source.as_bytes(),
            parser,
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

    fn line_range_of(node: Node) -> LineRange {
        LineRange {
            start_line: Self::line_of(node),
            end_line: node.end_position().row + 1,
        }
    }

    fn finish(self, had_syntax_errors: bool) -> FileSymbols {
        let mut merged: BTreeMap<FunctionId, FunctionDefinition> = BTreeMap::new();
        for clause in self.clauses {
            merged
                .entry(clause.id.clone())
                .and_modify(|function| {
                    function.start_line = function.start_line.min(clause.start_line);
                    function.end_line = function.end_line.max(clause.range.end_line);
                    function.clause_ranges.push(clause.range);
                })
                .or_insert_with(|| FunctionDefinition {
                    kind: clause.kind,
                    visibility: clause.visibility,
                    file_path: self.file_path.to_string(),
                    default_argument_count: 0,
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

    // -- items --------------------------------------------------------------

    /// Walks the items of a file or a `mod` body, which define names of their own.
    fn visit_module_items(
        &mut self,
        container: Node,
        scope: &mut Scope,
        enclosing: Option<LineRange>,
    ) {
        let names: Vec<String> = {
            let mut cursor = container.walk();
            container
                .named_children(&mut cursor)
                .filter_map(|item| self.item_name(item))
                .map(str::to_string)
                .collect()
        };
        scope.local_items.extend(names);
        self.visit_items(container, scope, enclosing);
    }

    /// The name an item defines in its module, if any.
    fn item_name(&self, item: Node) -> Option<&'a str> {
        let defines_name = matches!(
            item.kind(),
            "struct_item"
                | "enum_item"
                | "union_item"
                | "type_item"
                | "trait_item"
                | "mod_item"
                | "function_item"
                | "const_item"
                | "static_item"
        );
        defines_name
            .then(|| item.child_by_field_name("name"))
            .flatten()
            .map(|name| self.text(name))
    }

    /// Walks the items of a file, a `mod`, an `impl` or a `trait` body. With
    /// `enclosing`, the one function inside spans the whole block.
    fn visit_items(&mut self, container: Node, scope: &mut Scope, enclosing: Option<LineRange>) {
        let mut leading = LeadingRun::default();
        let mut cursor = container.walk();
        let items: Vec<Node> = container.named_children(&mut cursor).collect();
        for item in items {
            match item.kind() {
                "attribute_item" => leading.extend(item),
                "line_comment" | "block_comment" if self.is_outer_doc_comment(item) => {
                    leading.extend(item);
                }
                "line_comment" | "block_comment" => leading = LeadingRun::default(),
                "function_item" | "function_signature_item" => {
                    let (start_line, attributes) = leading.take_for(item);
                    let range = enclosing.unwrap_or_else(|| Self::line_range_of(item));
                    self.record_function(item, scope, start_line, &attributes, range);
                }
                "impl_item" => {
                    let (start_line, _) = leading.take_for(item);
                    self.visit_impl(item, scope, start_line);
                }
                "trait_item" => {
                    leading = LeadingRun::default();
                    self.visit_trait(item, scope);
                }
                "mod_item" => {
                    leading = LeadingRun::default();
                    self.visit_nested_module(item, scope);
                }
                "use_declaration" => {
                    leading = LeadingRun::default();
                    self.apply_use(item, scope);
                }
                _ => leading = LeadingRun::default(),
            }
        }
    }

    /// `///` and `/** */`, which document the item below.
    fn is_outer_doc_comment(&self, node: Node) -> bool {
        node.child_by_field_name("outer").is_some()
    }

    /// An `impl` with one function (`impl From<A> for B`, `impl Display for B`)
    /// is read as that function, header and all, from `leading_start_line`.
    fn visit_impl(&mut self, item: Node, scope: &Scope, leading_start_line: Option<usize>) {
        let (Some(type_node), Some(body)) = (
            item.child_by_field_name("type"),
            item.child_by_field_name("body"),
        ) else {
            return;
        };
        let Some(module) = self.implemented_type_module(type_node, scope) else {
            return;
        };
        let mut inner = scope.clone();
        inner.self_type = Some(module);

        let function_count = {
            let mut cursor = body.walk();
            body.named_children(&mut cursor)
                .filter(|child| child.kind() == "function_item")
                .count()
        };
        let whole_block = (function_count == 1).then(|| {
            let range = Self::line_range_of(item);
            LineRange {
                start_line: leading_start_line.unwrap_or(range.start_line),
                end_line: range.end_line,
            }
        });
        self.visit_items(body, &mut inner, whole_block);
    }

    /// The module path of the type an `impl` is for: `impl Repo` in `core::git`
    /// is `core::git::Repo`, and `impl Display for other::Thing` follows the path.
    fn implemented_type_module(&self, type_node: Node, scope: &Scope) -> Option<String> {
        let segments = self.type_path_segments(type_node)?;
        let last = segments.last()?;
        Some(match scope.resolve_path(&segments)? {
            PathTarget::Internal(path) | PathTarget::External(path) => path,
            PathTarget::Globbed(_) => format!("{}::{last}", scope.module),
        })
    }

    /// The path a type is written with, without generics, references or `dyn`.
    fn type_path_segments(&self, node: Node) -> Option<Vec<&'a str>> {
        match node.kind() {
            "type_identifier" | "primitive_type" => Some(vec![self.text(node)]),
            "scoped_type_identifier" => {
                let mut segments = node
                    .child_by_field_name("path")
                    .map(|path| self.path_segments(path))
                    .unwrap_or_default()?;
                segments.push(self.text(node.child_by_field_name("name")?));
                Some(segments)
            }
            "generic_type" | "reference_type" | "pointer_type" => {
                self.type_path_segments(node.child_by_field_name("type")?)
            }
            "dynamic_type" => self.type_path_segments(node.child_by_field_name("trait")?),
            _ => None,
        }
    }

    fn visit_trait(&mut self, item: Node, scope: &Scope) {
        let (Some(name), Some(body)) = (
            item.child_by_field_name("name"),
            item.child_by_field_name("body"),
        ) else {
            return;
        };
        let mut inner = scope.clone();
        inner.self_type = Some(format!("{}::{}", scope.module, self.text(name)));
        self.visit_items(body, &mut inner, None);
    }

    /// `mod name { ... }`. A `mod name;` has its items in another file.
    fn visit_nested_module(&mut self, item: Node, scope: &Scope) {
        let (Some(name), Some(body)) = (
            item.child_by_field_name("name"),
            item.child_by_field_name("body"),
        ) else {
            return;
        };
        let mut inner = scope.child_module(self.text(name));
        self.visit_module_items(body, &mut inner, None);
    }

    // -- use declarations ---------------------------------------------------

    fn apply_use(&mut self, item: Node, scope: &mut Scope) {
        if let Some(argument) = item.child_by_field_name("argument") {
            self.collect_use_tree(argument, &[], scope);
        }
    }

    /// Reads one `use` tree: `a::b`, `a::b as c`, `a::{b, c::d, self}` and `a::*`.
    fn collect_use_tree(&mut self, node: Node, prefix: &[&'a str], scope: &mut Scope) {
        let with_prefix = |segments: Vec<&'a str>| -> Vec<&'a str> {
            prefix.iter().copied().chain(segments).collect()
        };
        match node.kind() {
            "identifier" | "crate" | "super" => {
                let name = self.text(node);
                scope.add_use(name, &with_prefix(vec![name]));
            }
            "self" => {
                if let Some(last) = prefix.last() {
                    scope.add_use(last, prefix);
                }
            }
            "scoped_identifier" => {
                if let Some(segments) = self.path_segments(node) {
                    if let Some(last) = segments.last().copied() {
                        scope.add_use(last, &with_prefix(segments));
                    }
                }
            }
            "use_as_clause" => {
                let (Some(path), Some(alias)) = (
                    node.child_by_field_name("path"),
                    node.child_by_field_name("alias"),
                ) else {
                    return;
                };
                if let Some(segments) = self.path_segments(path) {
                    scope.add_use(self.text(alias), &with_prefix(segments));
                }
            }
            "scoped_use_list" => {
                let (Some(path), Some(list)) = (
                    node.child_by_field_name("path"),
                    node.child_by_field_name("list"),
                ) else {
                    return;
                };
                if let Some(segments) = self.path_segments(path) {
                    self.collect_use_tree(list, &with_prefix(segments), scope);
                }
            }
            "use_list" => {
                let mut cursor = node.walk();
                let members: Vec<Node> = node.named_children(&mut cursor).collect();
                for member in members {
                    self.collect_use_tree(member, prefix, scope);
                }
            }
            "use_wildcard" => {
                let segments = node
                    .named_child(0)
                    .and_then(|path| self.path_segments(path))
                    .unwrap_or_default();
                scope.add_glob(&with_prefix(segments));
            }
            _ => {}
        }
    }

    /// The segments of `a::b::c`, `crate::a`, `Self::new` or a lone name.
    /// `None` for paths tree-sitter cannot spell out (`<T as Trait>::f`).
    fn path_segments(&self, node: Node) -> Option<Vec<&'a str>> {
        match node.kind() {
            "identifier" | "crate" | "self" | "super" | "metavariable" => {
                Some(vec![self.text(node)])
            }
            "scoped_identifier" => {
                let mut segments = node
                    .child_by_field_name("path")
                    .map(|path| self.path_segments(path))
                    .unwrap_or_default()?;
                segments.push(self.text(node.child_by_field_name("name")?));
                Some(segments)
            }
            "generic_type" => self.path_segments(node.child_by_field_name("type")?),
            _ => None,
        }
    }

    // -- definitions --------------------------------------------------------

    fn record_function(
        &mut self,
        item: Node,
        scope: &Scope,
        leading_start_line: Option<usize>,
        attributes: &[Node],
        range: LineRange,
    ) {
        let Some(name_node) = item.child_by_field_name("name") else {
            return;
        };
        let arity = item
            .child_by_field_name("parameters")
            .map_or(0, |parameters| self.count_parameters(parameters));
        let is_test = attributes
            .iter()
            .any(|attribute| self.is_test_attribute(*attribute));
        let is_public = find_child(item, "visibility_modifier").is_some();

        let module = scope
            .self_type
            .clone()
            .unwrap_or_else(|| scope.module.clone());
        let id = FunctionId::new(module, self.text(name_node), arity);
        self.clauses.push(Clause {
            id: id.clone(),
            kind: if is_test {
                FunctionKind::Test
            } else {
                FunctionKind::Fn
            },
            visibility: if is_public {
                Visibility::Public
            } else {
                Visibility::Private
            },
            range,
            start_line: leading_start_line
                .unwrap_or(range.start_line)
                .min(range.start_line),
        });

        if let Some(body) = item.child_by_field_name("body") {
            let mut function_scope = scope.clone();
            self.walk_calls(body, &mut function_scope, &id);
        }
    }

    /// Parameters other than `self`.
    fn count_parameters(&self, parameters: Node) -> usize {
        let mut cursor = parameters.walk();
        parameters
            .named_children(&mut cursor)
            .filter(|parameter| !matches!(parameter.kind(), "self_parameter" | "attribute_item"))
            .count()
    }

    /// `#[test]`, `#[tokio::test]` and the like.
    fn is_test_attribute(&self, attribute_item: Node) -> bool {
        let Some(path) = find_child(attribute_item, "attribute").and_then(|a| a.named_child(0))
        else {
            return false;
        };
        self.path_segments(path)
            .and_then(|segments| segments.last().copied())
            == Some("test")
    }

    // -- calls inside function bodies ----------------------------------------

    fn walk_calls(&mut self, node: Node, scope: &mut Scope, caller: &FunctionId) {
        match node.kind() {
            "call_expression" => self.walk_call(node, scope, caller),
            "macro_invocation" => self.walk_macro_arguments(node, scope, caller),
            "use_declaration" => self.apply_use(node, scope),
            "scoped_identifier" => self.record_function_reference(node, scope, caller),
            "function_item" => {
                self.record_function(node, scope, None, &[], Self::line_range_of(node))
            }
            "impl_item" => self.visit_impl(node, scope, None),
            "trait_item" => self.visit_trait(node, scope),
            "mod_item" => self.visit_nested_module(node, scope),
            _ => self.walk_children(node, scope, caller),
        }
    }

    fn walk_children(&mut self, node: Node, scope: &mut Scope, caller: &FunctionId) {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.named_children(&mut cursor).collect();
        for child in children {
            self.walk_calls(child, scope, caller);
        }
    }

    fn walk_call(&mut self, call: Node, scope: &mut Scope, caller: &FunctionId) {
        let Some(function) = call.child_by_field_name("function") else {
            return self.walk_children(call, scope, caller);
        };
        let arity = call
            .child_by_field_name("arguments")
            .map_or(0, |arguments| self.count_arguments(arguments));
        self.record_call(function, arity, scope, caller);
        if let Some(arguments) = call.child_by_field_name("arguments") {
            self.walk_children(arguments, scope, caller);
        }
    }

    fn count_arguments(&self, arguments: Node) -> usize {
        let mut cursor = arguments.walk();
        arguments
            .named_children(&mut cursor)
            .filter(|argument| argument.kind() != "attribute_item")
            .count()
    }

    /// Records the call the callee expression makes, and walks the parts of
    /// it that may hold other calls (a method's receiver).
    fn record_call(
        &mut self,
        function: Node,
        arity: usize,
        scope: &mut Scope,
        caller: &FunctionId,
    ) {
        match function.kind() {
            "identifier" => self.record_bare_call(function, arity, scope, caller),
            "scoped_identifier" => {
                if let Some(segments) = self.path_segments(function) {
                    self.record_path_call(&segments, Some(arity), function, scope, caller);
                }
            }
            "generic_function" => {
                if let Some(inner) = function.child_by_field_name("function") {
                    self.record_call(inner, arity, scope, caller);
                }
            }
            "field_expression" => self.record_method_call(function, arity, scope, caller),
            _ => self.walk_calls(function, scope, caller),
        }
    }

    /// `name(args)`: a function of this module, a glob import, or one named in a `use`.
    fn record_bare_call(
        &mut self,
        name_node: Node,
        arity: usize,
        scope: &Scope,
        caller: &FunctionId,
    ) {
        let name = self.text(name_node);
        if let Some(full) = scope.uses.get(name) {
            if let Some((module, imported_name)) = full.rsplit_once("::") {
                let target = if scope.is_internal(module) {
                    RawCallTarget::ByName {
                        modules: vec![module.to_string()],
                        name: imported_name.to_string(),
                        arity: Some(arity),
                    }
                } else {
                    RawCallTarget::Remote {
                        module: module.to_string(),
                        name: imported_name.to_string(),
                        arity,
                    }
                };
                self.push_call(target, name_node, caller);
            }
            return;
        }
        self.push_call(
            RawCallTarget::Local {
                module: scope.module.clone(),
                name: name.to_string(),
                arity,
                imports: Arc::clone(&scope.globs),
            },
            name_node,
            caller,
        );
    }

    /// `a::b::name(args)`, or `a::b::name` used as a value when `arity` is `None`.
    fn record_path_call(
        &mut self,
        segments: &[&str],
        arity: Option<usize>,
        node: Node,
        scope: &Scope,
        caller: &FunctionId,
    ) {
        let Some((name, prefix)) = segments.split_last() else {
            return;
        };
        if prefix.is_empty() {
            return;
        }
        let target = match scope.resolve_path(prefix) {
            Some(PathTarget::Internal(module)) => RawCallTarget::ByName {
                modules: vec![module],
                name: name.to_string(),
                arity,
            },
            Some(PathTarget::Globbed(modules)) => RawCallTarget::ByName {
                modules,
                name: name.to_string(),
                arity,
            },
            Some(PathTarget::External(module)) => match arity {
                Some(arity) => RawCallTarget::Remote {
                    module,
                    name: name.to_string(),
                    arity,
                },
                None => return,
            },
            None => return,
        };
        self.push_call(target, node, caller);
    }

    /// `receiver.name(args)`. `self` and a type name pin the module; any
    /// other receiver is matched by name across the project.
    fn record_method_call(
        &mut self,
        field_expression: Node,
        arity: usize,
        scope: &mut Scope,
        caller: &FunctionId,
    ) {
        let (Some(receiver), Some(field)) = (
            field_expression.child_by_field_name("value"),
            field_expression.child_by_field_name("field"),
        ) else {
            return;
        };
        if field.kind() != "field_identifier" {
            return self.walk_calls(receiver, scope, caller);
        }
        let name = self.text(field);

        let modules = match receiver.kind() {
            "self" => scope.self_type.clone().map(|module| vec![module]),
            "identifier" if starts_uppercase(self.text(receiver)) => {
                match scope.resolve_path(&[self.text(receiver)]) {
                    Some(PathTarget::Internal(module)) => Some(vec![module]),
                    Some(PathTarget::Globbed(modules)) => Some(modules),
                    Some(PathTarget::External(_)) | None => None,
                }
            }
            _ => None,
        };
        let modules = modules.unwrap_or_default();
        if !modules.is_empty() || !COMMON_STD_METHOD_NAMES.contains(&name) {
            self.push_call(
                RawCallTarget::ByName {
                    modules,
                    name: name.to_string(),
                    arity: Some(arity),
                },
                field,
                caller,
            );
        }
        self.walk_calls(receiver, scope, caller);
    }

    /// `Self::helper` or `Type::helper` passed as a value, such as to `map`.
    fn record_function_reference(&mut self, node: Node, scope: &Scope, caller: &FunctionId) {
        let Some(segments) = self.path_segments(node) else {
            return;
        };
        if segments.last().is_some_and(|name| starts_lowercase(name)) {
            self.record_path_call(&segments, None, node, scope, caller);
        }
    }

    fn push_call(&mut self, target: RawCallTarget, node: Node, caller: &FunctionId) {
        self.calls.push(RawCall {
            caller: caller.clone(),
            target,
            line: Self::line_of(node),
        });
    }

    /// Parses the arguments of `name!(...)` as expressions, in place, so the
    /// calls in `assert_eq!(a.run(), 1)` are found with their real lines.
    /// Arguments that are not expressions parse to nothing useful and are skipped.
    fn walk_macro_arguments(&mut self, invocation: Node, scope: &mut Scope, caller: &FunctionId) {
        let Some(token_tree) = find_child(invocation, "token_tree") else {
            return;
        };
        if self
            .parser
            .set_included_ranges(&[token_tree.range()])
            .is_err()
        {
            return;
        }
        let source = self.source;
        let Some(tree) = self.parser.parse(source, None) else {
            return;
        };
        self.walk_children(tree.root_node(), scope, caller);
    }
}

fn find_child<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    let child = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind);
    child
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_at(path: &str, source: &str) -> FileSymbols {
        RustAnalyzer.parse_file(path, source).expect("parses")
    }

    fn parse(source: &str) -> FileSymbols {
        parse_at("app/src/shop/orders.rs", source)
    }

    fn by_name(modules: &[&str], name: &str, arity: Option<usize>) -> RawCallTarget {
        RawCallTarget::ByName {
            modules: modules.iter().map(|m| m.to_string()).collect(),
            name: name.to_string(),
            arity,
        }
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

    fn function_ids(symbols: &FileSymbols) -> Vec<String> {
        symbols.functions.iter().map(|f| f.id.to_string()).collect()
    }

    #[test]
    fn test_module_paths_follow_cargo_layout() {
        let module = |path: &str| module_path_of_file(path).1;
        assert_eq!(module("core/src/lib.rs"), "core");
        assert_eq!(module("core/src/main.rs"), "core");
        assert_eq!(module("core/src/analysis/rust.rs"), "core::analysis::rust");
        assert_eq!(module("core/src/analysis/mod.rs"), "core::analysis");
        assert_eq!(module("src/git.rs"), "crate::git");
        assert_eq!(module("core/tests/analysis.rs"), "core::tests::analysis");
        assert_eq!(module("tauri/src-tauri/build.rs"), "src-tauri::build");
    }

    #[test]
    fn test_functions_get_module_type_and_arity_without_self() {
        let symbols = parse(
            r#"pub struct Orders;

impl Orders {
    pub fn new() -> Self { Orders }
    fn place(&self, order: u32, note: &str) -> u32 { order }
}

pub fn helper(x: u32) -> u32 { x }

trait Summary {
    fn summarize(&self) -> String;
    fn title(&self) -> String { String::new() }
}
"#,
        );
        assert_eq!(
            function_ids(&symbols),
            vec![
                "app::shop::orders::Orders.new/0",
                "app::shop::orders::Orders.place/2",
                "app::shop::orders.helper/1",
                "app::shop::orders::Summary.summarize/0",
                "app::shop::orders::Summary.title/0",
            ]
        );
        assert_eq!(symbols.functions[0].visibility, Visibility::Public);
        assert_eq!(symbols.functions[1].visibility, Visibility::Private);
        assert_eq!(symbols.functions[0].kind, FunctionKind::Fn);
    }

    #[test]
    fn test_doc_comments_and_attributes_start_the_function() {
        let symbols = parse(
            r#"/// Places an order.
///
/// Twice.
#[inline]
pub fn place(order: u32) -> u32 {
    order
}

// A plain comment is not part of the function.
pub fn other() {}

#[derive(Debug)]
struct Gap;

fn after_gap() {}
"#,
        );
        let place = &symbols.functions[0];
        assert_eq!((place.start_line, place.end_line), (1, 7));
        assert_eq!(symbols.functions[1].start_line, 10);
        assert_eq!(symbols.functions[2].start_line, 15);
    }

    #[test]
    fn test_attributes_separated_by_a_blank_line_are_not_included() {
        let symbols = parse("#[allow(dead_code)]\n\nfn go() {}\n");
        assert_eq!(symbols.functions[0].start_line, 3);
    }

    #[test]
    fn test_test_functions_in_a_test_module_are_tests() {
        let symbols = parse(
            r#"pub fn place(order: u32) -> u32 { order }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_place() {
        assert_eq!(place(1), 1);
    }

    #[tokio::test]
    async fn test_async() {
        place(2);
    }
}
"#,
        );
        assert_eq!(
            function_ids(&symbols),
            vec![
                "app::shop::orders.place/1",
                "app::shop::orders::tests.test_place/0",
                "app::shop::orders::tests.test_async/0",
            ]
        );
        assert_eq!(symbols.functions[1].kind, FunctionKind::Test);
        assert_eq!(symbols.functions[2].kind, FunctionKind::Test);

        let targets = call_targets(&symbols, "test_place");
        let RawCallTarget::Local {
            module,
            name,
            arity,
            imports,
        } = &targets[0]
        else {
            panic!("expected a local call, got {targets:?}");
        };
        assert_eq!(
            (module.as_str(), name.as_str(), *arity),
            ("app::shop::orders::tests", "place", 1)
        );
        assert_eq!(imports[0].module, "app::shop::orders");
        assert!(imports[0].only.is_none());
        assert_eq!(
            symbols
                .calls
                .iter()
                .find(|c| c.caller.name == "test_place")
                .unwrap()
                .line,
            9
        );
    }

    #[test]
    fn test_paths_resolve_through_use_declarations_and_local_items() {
        let symbols = parse(
            r#"use crate::billing::Billing;
use crate::audit::{self, record as log_entry, Trail};
use super::helpers::*;
use std::fs;

struct Orders;

impl Orders {
    fn go(&self, x: u32) {
        Billing::charge(x, 1);
        audit::record(x);
        log_entry(x);
        Trail::start();
        Orders::new();
        Self::validate(x);
        self.total(x);
        super::parent_helper(x);
        crate::root_helper();
        fs::read_to_string("x");
        std::mem::swap(&mut 1, &mut 2);
        Shared::build();
        helper_from_glob(x);
    }
}
"#,
        );
        assert_eq!(
            call_targets(&symbols, "go"),
            vec![
                by_name(&["app::billing::Billing"], "charge", Some(2)),
                by_name(&["app::audit"], "record", Some(1)),
                by_name(&["app::audit"], "record", Some(1)),
                by_name(&["app::audit::Trail"], "start", Some(0)),
                by_name(&["app::shop::orders::Orders"], "new", Some(0)),
                by_name(&["app::shop::orders::Orders"], "validate", Some(1)),
                by_name(&["app::shop::orders::Orders"], "total", Some(1)),
                by_name(&["app::shop"], "parent_helper", Some(1)),
                by_name(&["app"], "root_helper", Some(0)),
                remote("std::fs", "read_to_string", 1),
                remote("std::mem", "swap", 2),
                by_name(&["app::shop::helpers::Shared"], "build", Some(0)),
                RawCallTarget::Local {
                    module: "app::shop::orders".to_string(),
                    name: "helper_from_glob".to_string(),
                    arity: 1,
                    imports: Arc::new(vec![ImportDirective {
                        module: "app::shop::helpers".to_string(),
                        only: None,
                    }]),
                },
            ]
        );
    }

    #[test]
    fn test_method_calls_on_values_match_by_name_except_common_std_names() {
        let symbols = parse(
            r#"fn go(orders: Orders, list: Vec<u32>) {
    orders.place(1);
    list.push(2);
    orders.items().len();
}
"#,
        );
        assert_eq!(
            call_targets(&symbols, "go"),
            vec![
                by_name(&[], "place", Some(1)),
                by_name(&[], "items", Some(0)),
            ]
        );
    }

    #[test]
    fn test_function_references_as_values_are_calls_without_arity() {
        let symbols = parse(
            r#"struct Orders;
impl Orders {
    fn go(&self, list: Vec<u32>) {
        list.iter().map(Self::convert).map(Orders::other).map(Kind::Added);
    }
}
"#,
        );
        assert_eq!(
            call_targets(&symbols, "go"),
            vec![
                by_name(&["app::shop::orders::Orders"], "convert", None),
                by_name(&["app::shop::orders::Orders"], "other", None),
            ]
        );
    }

    #[test]
    fn test_calls_inside_macro_arguments_are_found_with_their_lines() {
        let symbols = parse(
            r#"fn go() {
    assert_eq!(
        compute(1),
        2
    );
    println!("{}", Self::label());
    let v = vec![one(), two()];
}
"#,
        );
        let calls: Vec<(String, usize)> = symbols
            .calls
            .iter()
            .map(|call| match &call.target {
                RawCallTarget::Local { name, .. } => (name.clone(), call.line),
                RawCallTarget::ByName { name, .. } => (name.clone(), call.line),
                RawCallTarget::Remote { name, .. } => (name.clone(), call.line),
            })
            .collect();
        assert_eq!(
            calls,
            vec![
                ("compute".to_string(), 3),
                ("one".to_string(), 7),
                ("two".to_string(), 7),
            ]
        );
    }

    #[test]
    fn test_trait_impls_for_one_type_merge_same_named_methods() {
        let symbols = parse(
            r#"struct Id(u32);

impl From<u32> for Id {
    fn from(value: u32) -> Self { Id(value) }
}

impl From<i64> for Id {
    fn from(value: i64) -> Self { Id(value as u32) }
}
"#,
        );
        assert_eq!(function_ids(&symbols), vec!["app::shop::orders::Id.from/1"]);
        let from = &symbols.functions[0];
        assert_eq!(from.clause_ranges.len(), 2);
        assert_eq!((from.start_line, from.end_line), (3, 9));
    }

    #[test]
    fn test_an_impl_with_one_function_spans_the_whole_block() {
        let symbols = parse(
            r#"struct Id(u32);

/// Shown as a number.
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Id {
    /// Only the function.
    fn one(&self) {}
    fn two(&self) {}
}
"#,
        );
        let lines: Vec<(usize, usize)> = symbols
            .functions
            .iter()
            .map(|f| (f.start_line, f.end_line))
            .collect();
        assert_eq!(lines, vec![(3, 8), (11, 12), (13, 13)]);
    }

    #[test]
    fn test_impl_for_an_imported_type_uses_its_path() {
        let symbols =
            parse("use crate::model::Order;\nimpl Order {\n    fn total(&self) -> u32 { 0 }\n}\n");
        assert_eq!(function_ids(&symbols), vec!["app::model::Order.total/0"]);
    }

    #[test]
    fn test_syntax_errors_are_reported_but_parsing_continues() {
        let symbols = parse("fn ok() {}\nfn broken( {\n");
        assert!(symbols.had_syntax_errors);
        assert!(symbols.functions.iter().any(|f| f.id.name == "ok"));
    }
}

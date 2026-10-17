use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Identifies a function by module, name and full arity (including defaults).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FunctionId {
    pub module: String,
    pub name: String,
    pub arity: usize,
}

impl FunctionId {
    pub fn new(module: impl Into<String>, name: impl Into<String>, arity: usize) -> Self {
        Self {
            module: module.into(),
            name: name.into(),
            arity,
        }
    }
}

impl fmt::Display for FunctionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}/{}", self.module, self.name, self.arity)
    }
}

/// The keyword that defined a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionKind {
    Def,
    Defp,
    Defmacro,
    Defmacrop,
    Defguard,
    Defguardp,
    Defdelegate,
    /// An ExUnit `test` block, modeled as a zero-arity function.
    Test,
}

impl FunctionKind {
    pub fn visibility(self) -> Visibility {
        match self {
            Self::Defp | Self::Defmacrop | Self::Defguardp => Visibility::Private,
            _ => Visibility::Public,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
}

/// An inclusive range of 1-based line numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start_line: usize,
    pub end_line: usize,
}

impl LineRange {
    pub fn overlaps(&self, other: &LineRange) -> bool {
        self.start_line <= other.end_line && other.start_line <= self.end_line
    }
}

/// A function found in a source file, with every clause merged into one entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub id: FunctionId,
    pub kind: FunctionKind,
    pub visibility: Visibility,
    pub file_path: String,
    /// Number of trailing arguments with `\\` defaults.
    pub default_argument_count: usize,
    /// Lines from the first leading `@doc`/`@spec`/`@impl` through the last clause end.
    pub start_line: usize,
    pub end_line: usize,
    pub clause_ranges: Vec<LineRange>,
}

impl FunctionDefinition {
    pub fn line_range(&self) -> LineRange {
        LineRange {
            start_line: self.start_line,
            end_line: self.end_line,
        }
    }

    /// True when a call with `call_arity` arguments reaches this function.
    pub fn accepts_arity(&self, call_arity: usize) -> bool {
        let lowest = self.id.arity.saturating_sub(self.default_argument_count);
        (lowest..=self.id.arity).contains(&call_arity)
    }
}

/// An `import` directive in scope where a call is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDirective {
    pub module: String,
    /// `only: [name: arity]` pairs; `None` imports every public function.
    pub only: Option<Vec<(String, usize)>>,
}

impl ImportDirective {
    pub fn brings_into_scope(&self, name: &str, arity: usize) -> bool {
        match &self.only {
            None => true,
            Some(pairs) => pairs
                .iter()
                .any(|(only_name, only_arity)| only_name == name && *only_arity == arity),
        }
    }
}

/// A call as written in source, before it is matched to a definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawCallTarget {
    /// `Module.function(args)` with the module name already alias-resolved.
    Remote {
        module: String,
        name: String,
        arity: usize,
    },
    /// `function(args)` in `module`, which may be a local or imported function.
    Local {
        module: String,
        name: String,
        arity: usize,
        imports: Arc<Vec<ImportDirective>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCall {
    pub caller: FunctionId,
    pub target: RawCallTarget,
    pub line: usize,
}

/// Everything extracted from one parsed source file.
#[derive(Debug, Clone, Default)]
pub struct FileSymbols {
    pub file_path: String,
    pub functions: Vec<FunctionDefinition>,
    pub calls: Vec<RawCall>,
    /// True when tree-sitter recovered from syntax errors in the file.
    pub had_syntax_errors: bool,
}

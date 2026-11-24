use super::elixir::ElixirAnalyzer;
use super::error::AnalysisError;
use super::model::FileSymbols;
use super::rust::RustAnalyzer;

/// Extracts functions and calls from source files of one language.
///
/// Each language implements this with its own tree-sitter grammar. Everything
/// after parsing (index, call graph, traces) is language independent.
pub trait LanguageAnalyzer: Send + Sync {
    fn language_name(&self) -> &'static str;

    /// File extensions without the dot.
    fn file_extensions(&self) -> &'static [&'static str];

    fn parse_file(&self, file_path: &str, source: &str) -> Result<FileSymbols, AnalysisError>;
}

static ELIXIR_ANALYZER: ElixirAnalyzer = ElixirAnalyzer;
static RUST_ANALYZER: RustAnalyzer = RustAnalyzer;
static ANALYZERS: &[&dyn LanguageAnalyzer] = &[&ELIXIR_ANALYZER, &RUST_ANALYZER];

/// Finds the analyzer for a file path by its extension.
pub fn find_analyzer_for_path(file_path: &str) -> Option<&'static dyn LanguageAnalyzer> {
    let extension = file_path.rsplit_once('.')?.1;
    ANALYZERS
        .iter()
        .copied()
        .find(|analyzer| analyzer.file_extensions().contains(&extension))
}

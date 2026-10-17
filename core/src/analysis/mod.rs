//! Static code analysis: parse source with tree-sitter, index functions and
//! calls, then answer questions about changed functions and call paths.
//!
//! Language support is one `LanguageAnalyzer` per language. Everything else
//! is shared.

pub mod call_trace;
pub mod changed_functions;
mod elixir;
pub mod error;
pub mod external_calls;
pub mod function_changes;
pub mod function_source;
pub mod language;
pub mod model;
pub mod project_index;
pub mod service;

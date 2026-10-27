//! Code Reviewer's shared logic: reads a git repository, indexes its Elixir
//! code with tree-sitter and answers what a comparison changed, function by
//! function and file by file. The desktop app wraps it in Tauri commands; a
//! web server can wrap the same functions later.

pub mod analysis;
pub mod git;
pub mod groups;

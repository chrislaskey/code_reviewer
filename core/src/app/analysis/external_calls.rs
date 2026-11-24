use serde::{Deserialize, Serialize};

use super::changed_functions::ChangedFunction;
use super::error::AnalysisError;
use super::model::FunctionId;
use super::project_index::{CallLink, ProjectIndex};

/// Calls crossing the module boundary of one function, one level deep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCalls {
    pub function: FunctionId,
    /// Functions in other modules that call this function.
    pub calls_in: Vec<CallLink>,
    /// Functions in other modules that this function calls.
    pub calls_out: Vec<CallLink>,
}

/// List the direct calls into and out of `function` that cross a module boundary.
pub fn list_external_calls_for_function(
    index: &ProjectIndex,
    function: &FunctionId,
) -> Result<ExternalCalls, AnalysisError> {
    if index.get_function(function).is_none() {
        return Err(AnalysisError::FunctionNotFound(function.to_string()));
    }

    let only_external = |links: Vec<CallLink>| -> Vec<CallLink> {
        links
            .into_iter()
            .filter(|link| link.crosses_module_boundary)
            .collect()
    };
    Ok(ExternalCalls {
        function: function.clone(),
        calls_in: only_external(index.list_incoming_call_links(function)),
        calls_out: only_external(index.list_outgoing_call_links(function)),
    })
}

/// One-level external calls for every changed function.
pub fn list_external_calls_for_changed_functions(
    index: &ProjectIndex,
    changed_functions: &[ChangedFunction],
) -> Result<Vec<ExternalCalls>, AnalysisError> {
    changed_functions
        .iter()
        .map(|changed| list_external_calls_for_function(index, &changed.function))
        .collect()
}

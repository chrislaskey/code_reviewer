use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::error::AnalysisError;
use super::model::FunctionId;
use super::project_index::{CallDirection, ProjectIndex};

const DEFAULT_MAX_NODE_COUNT: usize = 5_000;

/// Request for the full call tree above or below one function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCallTraceRequest {
    pub function: FunctionId,
    pub direction: CallDirection,
    /// Levels below the root to follow. `None` follows until cycles or dead ends.
    pub max_depth: Option<usize>,
    /// Stops expanding after this many nodes. Defaults to 5000.
    pub max_node_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStopReason {
    /// The function is already on the path from the root, so following it would loop.
    Cycle,
    MaxDepth,
    MaxNodeCount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallTraceNode {
    pub function: FunctionId,
    /// False for functions outside the project, which are never expanded.
    pub defined_in_project: bool,
    /// File and lines of the call that led here. Empty on the root.
    pub call_file_path: Option<String>,
    pub call_lines: Vec<usize>,
    pub crosses_module_boundary: bool,
    pub stop_reason: Option<TraceStopReason>,
    pub children: Vec<CallTraceNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetCallTraceResponse {
    pub root: CallTraceNode,
    pub node_count: usize,
    /// True when `max_depth` or `max_node_count` cut the tree short.
    pub truncated: bool,
}

struct Tracer<'a> {
    index: &'a ProjectIndex,
    direction: CallDirection,
    max_depth: Option<usize>,
    max_node_count: usize,
    node_count: usize,
    truncated: bool,
    path: HashSet<FunctionId>,
}

/// Follow every call above or below a function, local and external, through the whole project.
pub fn get_call_trace(
    index: &ProjectIndex,
    request: GetCallTraceRequest,
) -> Result<GetCallTraceResponse, AnalysisError> {
    if index.get_function(&request.function).is_none() {
        return Err(AnalysisError::FunctionNotFound(
            request.function.to_string(),
        ));
    }

    let mut tracer = Tracer {
        index,
        direction: request.direction,
        max_depth: request.max_depth,
        max_node_count: request.max_node_count.unwrap_or(DEFAULT_MAX_NODE_COUNT),
        node_count: 1,
        truncated: false,
        path: HashSet::new(),
    };
    let mut root = CallTraceNode {
        function: request.function,
        defined_in_project: true,
        call_file_path: None,
        call_lines: Vec::new(),
        crosses_module_boundary: false,
        stop_reason: None,
        children: Vec::new(),
    };
    tracer.expand(&mut root, 0);

    Ok(GetCallTraceResponse {
        root,
        node_count: tracer.node_count,
        truncated: tracer.truncated,
    })
}

impl Tracer<'_> {
    fn expand(&mut self, node: &mut CallTraceNode, depth: usize) {
        if !node.defined_in_project {
            return;
        }
        if self.max_depth.is_some_and(|max| depth >= max) {
            self.stop(node, TraceStopReason::MaxDepth);
            return;
        }

        let links = self.index.list_call_links(&node.function, self.direction);
        if links.is_empty() {
            return;
        }

        self.path.insert(node.function.clone());
        for link in links {
            if self.node_count >= self.max_node_count {
                self.stop(node, TraceStopReason::MaxNodeCount);
                break;
            }
            self.node_count += 1;

            let mut child = CallTraceNode {
                function: link.function,
                defined_in_project: link.defined_in_project,
                call_file_path: Some(link.call_file_path),
                call_lines: link.call_lines,
                crosses_module_boundary: link.crosses_module_boundary,
                stop_reason: None,
                children: Vec::new(),
            };
            if self.path.contains(&child.function) {
                child.stop_reason = Some(TraceStopReason::Cycle);
            } else {
                self.expand(&mut child, depth + 1);
            }
            node.children.push(child);
        }
        self.path.remove(&node.function);
    }

    fn stop(&mut self, node: &mut CallTraceNode, reason: TraceStopReason) {
        self.truncated = true;
        node.stop_reason = Some(reason);
    }
}

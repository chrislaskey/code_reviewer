//! The calls a test makes, walked top to bottom like a stack trace. Each
//! project function on the way is shown in full, with its lines marked
//! against the base of the comparison.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::call_trace::TraceStopReason;
use super::error::AnalysisError;
use super::function_changes::FunctionChangeKind;
use super::function_diff::{diff_function_sources, DiffLine};
use super::function_source::{extract_function_source, FunctionSource};
use super::model::{FunctionId, FunctionKind};
use super::project_index::{CallLink, ProjectIndex};

const DEFAULT_MAX_FRAME_COUNT: usize = 2_000;

/// One ExUnit test in the head of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSummary {
    pub function: FunctionId,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Guessed from the file path (`/live/`) or module name (`LiveTest`).
    pub is_live_view_test: bool,
    /// Set when the test itself was added or modified in the comparison.
    pub change: Option<FunctionChangeKind>,
}

/// How far to walk from the test.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestCallStackOptions {
    /// Levels below the test to follow. `None` follows until every function was shown once.
    pub max_depth: Option<usize>,
    /// Stops walking after this many frames. Defaults to 2000.
    pub max_frame_count: Option<usize>,
}

/// One function on the walk from the test, in the order it would be entered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallStackFrame {
    pub function: FunctionId,
    /// Zero on the test, one more for each call below it.
    pub depth: usize,
    /// False for functions outside the project (deps, standard library), which have no source here.
    pub defined_in_project: bool,
    /// Where the function is defined. `None` outside the project.
    pub file_path: Option<String>,
    /// File and lines of the call that led here. Empty on the test.
    pub call_file_path: Option<String>,
    pub call_lines: Vec<usize>,
    pub crosses_module_boundary: bool,
    /// `None` when the function reads the same in the base, or is outside the project.
    pub change: Option<FunctionChangeKind>,
    /// The whole function in the head, each line marked against the base. Empty outside the project.
    pub lines: Vec<DiffLine>,
    /// True when the function was already walked above, so its calls are not listed again.
    pub repeated: bool,
    pub stop_reason: Option<TraceStopReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCallStack {
    pub test: FunctionId,
    pub frames: Vec<CallStackFrame>,
    /// True when `max_depth` or `max_frame_count` cut the walk short.
    pub truncated: bool,
}

fn looks_like_live_view_test(function: &FunctionId, file_path: &str) -> bool {
    file_path.contains("/live/") || function.module.ends_with("LiveTest")
}

/// Every ExUnit test in the index, by file and then by line.
pub fn list_tests(
    index: &ProjectIndex,
    changed_functions: &HashMap<FunctionId, FunctionChangeKind>,
) -> Vec<TestSummary> {
    let mut tests: Vec<TestSummary> = index
        .list_functions()
        .filter(|definition| definition.kind == FunctionKind::Test)
        .map(|definition| TestSummary {
            function: definition.id.clone(),
            file_path: definition.file_path.clone(),
            start_line: definition.start_line,
            end_line: definition.end_line,
            is_live_view_test: looks_like_live_view_test(&definition.id, &definition.file_path),
            change: changed_functions.get(&definition.id).copied(),
        })
        .collect();
    tests.sort_by(|a, b| (&a.file_path, a.start_line).cmp(&(&b.file_path, b.start_line)));
    tests
}

/// Reads function sources from one index, keeping each file's text after the first read.
struct SourceReader<'a> {
    index: &'a ProjectIndex,
    file_texts: HashMap<String, String>,
}

impl<'a> SourceReader<'a> {
    fn new(index: &'a ProjectIndex) -> Self {
        Self {
            index,
            file_texts: HashMap::new(),
        }
    }

    /// `None` when the index does not define the function.
    fn get_function_source(
        &mut self,
        function: &FunctionId,
    ) -> Result<Option<FunctionSource>, AnalysisError> {
        let Some(definition) = self.index.get_function(function) else {
            return Ok(None);
        };
        if !self.file_texts.contains_key(&definition.file_path) {
            let text = self.index.read_file_text(&definition.file_path)?;
            self.file_texts.insert(definition.file_path.clone(), text);
        }
        let text = &self.file_texts[&definition.file_path];
        Ok(Some(extract_function_source(definition, text)))
    }
}

struct Walker<'a> {
    head: SourceReader<'a>,
    base: SourceReader<'a>,
    max_depth: Option<usize>,
    max_frame_count: usize,
    frames: Vec<CallStackFrame>,
    truncated: bool,
    walked: HashSet<FunctionId>,
    path: HashSet<FunctionId>,
}

/// Walk every call below the test through the head index, depth first and in
/// call order, marking each project function against the base index.
pub fn get_test_call_stack(
    base_index: &ProjectIndex,
    head_index: &ProjectIndex,
    test: &FunctionId,
    options: TestCallStackOptions,
) -> Result<TestCallStack, AnalysisError> {
    if head_index.get_function(test).is_none() {
        return Err(AnalysisError::FunctionNotFound(test.to_string()));
    }

    let mut walker = Walker {
        head: SourceReader::new(head_index),
        base: SourceReader::new(base_index),
        max_depth: options.max_depth,
        max_frame_count: options.max_frame_count.unwrap_or(DEFAULT_MAX_FRAME_COUNT),
        frames: Vec::new(),
        truncated: false,
        walked: HashSet::new(),
        path: HashSet::new(),
    };
    let root = CallLink {
        function: test.clone(),
        defined_in_project: true,
        call_file_path: String::new(),
        call_lines: Vec::new(),
        crosses_module_boundary: false,
    };
    walker.walk(&root, 0)?;

    Ok(TestCallStack {
        test: test.clone(),
        frames: walker.frames,
        truncated: walker.truncated,
    })
}

/// Links in the order the calls appear in the source. Calls on one line are not
/// told apart, so there project functions go first.
fn sorted_by_first_call_line(mut links: Vec<CallLink>) -> Vec<CallLink> {
    links.sort_by_key(|link| {
        (
            link.call_lines.first().copied().unwrap_or(0),
            !link.defined_in_project,
        )
    });
    links
}

impl Walker<'_> {
    fn walk(&mut self, link: &CallLink, depth: usize) -> Result<(), AnalysisError> {
        if self.frames.len() >= self.max_frame_count {
            self.truncated = true;
            return Ok(());
        }

        let mut frame = self.frame_for(link, depth)?;
        let frame_index = self.frames.len();
        if !frame.defined_in_project {
            self.frames.push(frame);
            return Ok(());
        }

        if self.path.contains(&link.function) {
            frame.repeated = true;
            frame.stop_reason = Some(TraceStopReason::Cycle);
            self.frames.push(frame);
            return Ok(());
        }
        if !self.walked.insert(link.function.clone()) {
            frame.repeated = true;
            self.frames.push(frame);
            return Ok(());
        }
        if self.max_depth.is_some_and(|max| depth >= max) {
            let has_calls = !self
                .head
                .index
                .list_outgoing_call_links(&link.function)
                .is_empty();
            if has_calls {
                frame.stop_reason = Some(TraceStopReason::MaxDepth);
                self.truncated = true;
            }
            self.frames.push(frame);
            return Ok(());
        }
        self.frames.push(frame);

        let links =
            sorted_by_first_call_line(self.head.index.list_outgoing_call_links(&link.function));
        self.path.insert(link.function.clone());
        for child in &links {
            if self.frames.len() >= self.max_frame_count {
                self.truncated = true;
                self.frames[frame_index].stop_reason = Some(TraceStopReason::MaxNodeCount);
                break;
            }
            self.walk(child, depth + 1)?;
        }
        self.path.remove(&link.function);
        Ok(())
    }

    fn frame_for(
        &mut self,
        link: &CallLink,
        depth: usize,
    ) -> Result<CallStackFrame, AnalysisError> {
        let call_file_path = (depth > 0).then(|| link.call_file_path.clone());
        let mut frame = CallStackFrame {
            function: link.function.clone(),
            depth,
            defined_in_project: link.defined_in_project,
            file_path: None,
            call_file_path,
            call_lines: link.call_lines.clone(),
            crosses_module_boundary: link.crosses_module_boundary,
            change: None,
            lines: Vec::new(),
            repeated: false,
            stop_reason: None,
        };
        if !link.defined_in_project {
            return Ok(frame);
        }

        let Some(new) = self.head.get_function_source(&link.function)? else {
            return Ok(frame);
        };
        let old = self.base.get_function_source(&link.function)?;
        frame.change = match &old {
            None => Some(FunctionChangeKind::Added),
            Some(old)
                if old
                    .lines
                    .iter()
                    .map(|line| &line.text)
                    .ne(new.lines.iter().map(|line| &line.text)) =>
            {
                Some(FunctionChangeKind::Modified)
            }
            Some(_) => None,
        };
        frame.file_path = Some(new.file_path.clone());
        frame.lines = diff_function_sources(old.as_ref(), Some(&new));
        Ok(frame)
    }
}

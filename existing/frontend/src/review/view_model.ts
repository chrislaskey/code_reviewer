import type {
  FileChange,
  FileDiffTarget,
  FileFunctionChange,
  FunctionDiffTarget,
  FunctionId,
  GroupRow,
} from "../api/types";
import type { GroupRow as ViewGroupRow } from "../explorations/group_rows";
import type { GroupedFile } from "../explorations/groups_data";

/*
 * Maps the backend's file changes onto the shape the view builders draw.
 * Fields the backend has no data for are left out, so the views hide them.
 */

function functionKey(function_id: FunctionId): string {
  return `${function_id.name}/${function_id.arity}`;
}

function toFileDiffTarget(file: FileChange): FileDiffTarget {
  switch (file.status) {
    case "added":
      return { old_path: null, new_path: file.file_path };
    case "removed":
      return { old_path: file.file_path, new_path: null };
    case "modified":
      return { old_path: file.previous_path ?? file.file_path, new_path: file.file_path };
  }
}

function toDiffTarget(change: FileFunctionChange): FunctionDiffTarget {
  switch (change.kind) {
    case "added":
      return { old_function: null, new_function: change.function };
    case "removed":
      return { old_function: change.function, new_function: null };
    case "renamed":
      return { old_function: change.previous_function, new_function: change.function };
    case "modified":
      return { old_function: change.function, new_function: change.function };
  }
}

export function toViewFiles(files: FileChange[]): GroupedFile[] {
  return files.map((file, index) => ({
    id: `file-${index}`,
    group_ids: file.group_ids,
    path: file.file_path,
    module: file.module,
    status: file.status,
    plain: file.kind === "unparsed",
    file_diff_target: file.kind === "unparsed" ? toFileDiffTarget(file) : undefined,
    functions: file.functions.map((change) => ({
      name: change.function.name,
      arity: change.function.arity,
      kind: change.kind,
      added_lines: change.added_line_count ?? undefined,
      removed_lines: change.removed_line_count ?? undefined,
      previous_name: change.previous_function
        ? functionKey(change.previous_function)
        : undefined,
      diff_target: toDiffTarget(change),
    })),
    callers: file.callers.map((caller) => ({
      module: caller.function.module,
      name: caller.function.name,
      arity: caller.function.arity,
      file_path: caller.file_path,
      line: caller.line,
      kind: "production",
      calls: functionKey(caller.calls),
      stale: caller.is_stale,
      also_changed: caller.is_also_changed,
    })),
    callees: file.callees.map((callee) => ({
      module: callee.function.module,
      name: callee.function.name,
      arity: callee.function.arity,
      origin: callee.is_highlighted ? "project" : "dependency",
      from: functionKey(callee.from),
      call_count: callee.call_count,
      diff: "same",
    })),
  }));
}

/** Group rows name files by path. The views name them by the id of `toViewFiles`. */
export function toViewGroupRows(rows: GroupRow[], files: GroupedFile[]): ViewGroupRow[] {
  const idByPath = new Map(files.map((file) => [file.path, file.id]));
  return rows.map((row) => ({
    ...row,
    lines: row.lines.map((line) => ({
      ...line,
      id: line.kind === "file" ? (idByPath.get(line.id) ?? line.id) : line.id,
    })),
    outputs: row.outputs.map(toViewLink(idByPath)),
    inputs: row.inputs.map(toViewLink(idByPath)),
  }));
}

function toViewLink(idByPath: Map<string, string>) {
  return (link: GroupRow["outputs"][number]): ViewGroupRow["outputs"][number] => ({
    ...link,
    file_id: link.changed_file_path ? (idByPath.get(link.changed_file_path) ?? null) : null,
    is_test: false,
  });
}

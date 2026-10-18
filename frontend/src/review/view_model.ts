import type { FileChange, FunctionId, GroupRow } from "../api/types";
import type { GroupRow as ViewGroupRow } from "../explorations/group_rows";
import type { GroupedFile } from "../explorations/groups_data";

/*
 * Maps the backend's file changes onto the shape the view builders draw.
 * Fields the backend has no data for are left out, so the views hide them.
 */

function functionKey(function_id: FunctionId): string {
  return `${function_id.name}/${function_id.arity}`;
}

export function toViewFiles(files: FileChange[]): GroupedFile[] {
  return files.map((file, index) => ({
    id: `file-${index}`,
    group_ids: file.group_ids,
    path: file.file_path,
    module: file.module,
    status: file.status,
    functions: file.functions.map((change) => ({
      name: change.function.name,
      arity: change.function.arity,
      kind: change.kind,
      added_lines: change.added_line_count ?? undefined,
      removed_lines: change.removed_line_count ?? undefined,
      previous_name: change.previous_function
        ? functionKey(change.previous_function)
        : undefined,
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

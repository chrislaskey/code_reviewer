// Maps the backend's file changes and group rows onto what the views draw.
// Files get short ids, functions and callers get `name/arity` keys, and the
// group rows name files by those ids.

export function functionKey({ name, arity }) {
  return `${name}/${arity}`;
}

export function functionLabel({ module, name, arity }) {
  return `${module}.${name}/${arity}`;
}

/** The last two parts of a path. */
export function shortPath(path) {
  return path.split("/").slice(-2).join("/");
}

export function fileName(path) {
  return path.split("/").pop() ?? path;
}

/** The id of the row that rolls up every group. Mirrors the core constant. */
export const ALL_CHANGES_ID = "@all";

/** The implicit group of files no group holds. */
export const UNGROUPED_ID = "ungrouped";

function toFileDiffTarget(file) {
  switch (file.status) {
    case "added":
      return { old_path: null, new_path: file.file_path };
    case "removed":
      return { old_path: file.file_path, new_path: null };
    default:
      return { old_path: file.previous_path ?? file.file_path, new_path: file.file_path };
  }
}

function toFunctionDiffTarget(change) {
  switch (change.kind) {
    case "added":
      return { old_function: null, new_function: change.function };
    case "removed":
      return { old_function: change.function, new_function: null };
    case "renamed":
      return { old_function: change.previous_function, new_function: change.function };
    default:
      return { old_function: change.function, new_function: change.function };
  }
}

/** The backend's `files` as the views draw them. */
export function toViewFiles(files) {
  return files.map((file, index) => ({
    id: `file-${index}`,
    path: file.file_path,
    module: file.module,
    status: file.status,
    group_ids: file.group_ids,
    /** A file that was not parsed: no functions, callers or calls, only its text. */
    plain: file.kind === "unparsed",
    file_diff_target: file.kind === "unparsed" ? toFileDiffTarget(file) : null,
    functions: file.functions.map((change) => ({
      key: functionKey(change.function),
      kind: change.kind,
      added_lines: change.added_line_count,
      removed_lines: change.removed_line_count,
      previous_key: change.previous_function ? functionKey(change.previous_function) : null,
      diff_target: toFunctionDiffTarget(change),
    })),
    callers: file.callers.map((caller) => ({
      label: functionLabel(caller.function),
      file_path: caller.file_path,
      line: caller.line,
      calls: functionKey(caller.calls),
      stale: caller.is_stale,
      also_changed: caller.is_also_changed,
    })),
    callees: file.callees.map((callee) => ({
      label: functionLabel(callee.function),
      /** Defined in the project, or matched by an include rule. The rest are folded. */
      highlighted: callee.is_highlighted,
      file_path: callee.file_path,
      from: functionKey(callee.from),
      call_count: callee.call_count,
      also_changed: callee.is_also_changed,
    })),
  }));
}

/** Group rows name files by path. The views name them by the id of `toViewFiles`. */
export function toViewGroupRows(rows, files) {
  const idByPath = new Map(files.map((file) => [file.path, file.id]));
  const toLink = (link) => ({
    ...link,
    file_id: link.changed_file_path ? (idByPath.get(link.changed_file_path) ?? null) : null,
  });
  return rows.map((row) => ({
    ...row,
    lines: row.lines.map((line) => ({
      ...line,
      id: line.kind === "file" ? (idByPath.get(line.id) ?? line.id) : line.id,
    })),
    outputs: row.outputs.map(toLink),
    inputs: row.inputs.map(toLink),
  }));
}

export function filesInGroup(files, groupId) {
  return files.filter((file) => file.group_ids.includes(groupId));
}

export function findFileByPath(files, path) {
  return files.find((file) => file.path === path) ?? null;
}

export function findFileById(files, fileId) {
  return files.find((file) => file.id === fileId) ?? null;
}

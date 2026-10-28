function claudeCodeStatusOf(session) {
  return session.claude_code ? (session.claude_code.status ?? "unknown") : "not running";
}

function claudeCodeStatusByEntry(sessionList) {
  const statuses = new Map();
  for (const session of sessionList.sessions) {
    statuses.set(`session:${session.name}`, { name: session.name, status: claudeCodeStatusOf(session) });
  }
  for (const worktree of sessionList.worktrees) {
    statuses.set(`worktree:${worktree.path}`, { name: worktree.name, status: undefined });
  }
  return statuses;
}

export function listSessionChanges(previous, next) {
  const before = claudeCodeStatusByEntry(previous);
  const after = claudeCodeStatusByEntry(next);
  const changes = { added: [], removed: [], statusChanged: [] };
  for (const [entry, { name, status }] of after) {
    if (!before.has(entry)) changes.added.push(name);
    else if (before.get(entry).status !== status) {
      changes.statusChanged.push({ name, from: before.get(entry).status, to: status });
    }
  }
  for (const [entry, { name }] of before) {
    if (!after.has(entry)) changes.removed.push(name);
  }
  return changes;
}

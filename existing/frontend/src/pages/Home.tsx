import { useState } from "react";
import { Link } from "react-router-dom";
import type { ChangedFile } from "../api/types";
import { useBackend } from "../api/context";

const REPO_PATH = "/Users/chrislaskey/code/code_reviewer";

export default function Home() {
  const backend = useBackend();
  const [files, setFiles] = useState<ChangedFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleListFiles() {
    setLoading(true);
    setError(null);
    try {
      const response = await backend.listChangedFiles({
        repo_path: REPO_PATH,
      });
      setFiles(response.files);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="mx-auto max-w-2xl px-6 py-12">
      <h1 className="text-3xl font-bold mb-2">Code Reviewer</h1>
      <p className="text-base-content/60 mb-2">
        Shared UI running on both Tauri and Axum.
      </p>
      <p className="mb-2 text-sm">
        <Link to="/review" className="text-primary hover:underline">
          Review changes
        </Link>
      </p>
      <p className="mb-8 text-sm">
        <Link to="/explorations" className="text-primary hover:underline">
          UI explorations
        </Link>
      </p>

      <button
        onClick={handleListFiles}
        disabled={loading}
        className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
      >
        {loading ? "Loading…" : "Show Changed Files"}
      </button>

      {error && (
        <p className="mt-4 text-red-600 text-sm">Error: {error}</p>
      )}

      {files.length > 0 && (
        <div className="mt-6">
          <h2 className="text-lg font-semibold mb-3">
            Changed Files ({files.length})
          </h2>
          <ul className="divide-y divide-base-300 rounded-lg border border-base-300">
            {files.map((file) => (
              <li
                key={file.path}
                className="flex items-center justify-between px-4 py-2 text-sm"
              >
                <code className="font-mono text-base-content">{file.path}</code>
                <StatusBadge status={file.status} />
              </li>
            ))}
          </ul>
        </div>
      )}

      {!loading && !error && files.length === 0 && (
        <p className="mt-6 text-base-content/40 text-sm">
          Press the button to list git-changed files.
        </p>
      )}
    </main>
  );
}

function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    modified: "bg-amber-500/15 text-amber-700 dark:text-amber-300",
    added: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300",
    deleted: "bg-rose-500/15 text-rose-700 dark:text-rose-300",
    untracked: "bg-base-content/10 text-base-content/60",
    renamed: "bg-violet-500/15 text-violet-700 dark:text-violet-300",
  };
  const cls = colors[status] ?? "bg-base-content/10 text-base-content/60";
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${cls}`}>
      {status}
    </span>
  );
}

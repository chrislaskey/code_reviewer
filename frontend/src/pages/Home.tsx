import { useState } from "react";
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
      <p className="text-gray-500 mb-8">
        Shared UI running on both Tauri and Axum.
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
          <ul className="divide-y divide-gray-200 rounded-lg border border-gray-200">
            {files.map((file) => (
              <li
                key={file.path}
                className="flex items-center justify-between px-4 py-2 text-sm"
              >
                <code className="font-mono text-gray-800">{file.path}</code>
                <StatusBadge status={file.status} />
              </li>
            ))}
          </ul>
        </div>
      )}

      {!loading && !error && files.length === 0 && (
        <p className="mt-6 text-gray-400 text-sm">
          Press the button to list git-changed files.
        </p>
      )}
    </main>
  );
}

function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    modified: "bg-yellow-100 text-yellow-800",
    added: "bg-green-100 text-green-800",
    deleted: "bg-red-100 text-red-800",
    untracked: "bg-gray-100 text-gray-600",
    renamed: "bg-purple-100 text-purple-800",
  };
  const cls = colors[status] ?? "bg-gray-100 text-gray-600";
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${cls}`}>
      {status}
    </span>
  );
}

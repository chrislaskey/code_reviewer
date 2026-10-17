import { useMemo, useRef, useState, type FormEvent } from "react";
import { useBackend } from "../../api/context";
import type { AnalyzeFileChangesResponse } from "../../api/types";
import { OUTSIDE_MODE_OPTIONS, type OutsideMode } from "../../explorations/data";
import { MarkerIcon } from "../../explorations/nodes";
import { SegmentedControl, Toolbar } from "../../explorations/controls";
import { Legend } from "../../explorations/scaffold";
import {
  REVIEW_VIEW_OPTIONS,
  ReviewCanvas,
  type ReviewView,
} from "../../review/ReviewCanvas";
import { toViewFiles } from "../../review/view_model";

const STORAGE_KEY = "code_reviewer.review_form";

interface ReviewForm {
  repoPath: string;
  baseRef: string;
  headRef: string;
}

function loadForm(): ReviewForm {
  const empty = { repoPath: "", baseRef: "", headRef: "" };
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return saved ? { ...empty, ...JSON.parse(saved) } : empty;
  } catch {
    return empty;
  }
}

function saveForm(form: ReviewForm) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(form));
  } catch {
    // Remembering the form is a convenience only.
  }
}

function TextField({
  label,
  value,
  placeholder,
  className,
  onChange,
}: {
  label: string;
  value: string;
  placeholder: string;
  className?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className={`flex flex-col gap-1 text-[11px] text-base-content/60 ${className ?? ""}`}>
      {label}
      <input
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        className="rounded-lg border border-base-300 bg-base-100 px-2.5 py-1.5 font-mono text-xs text-base-content placeholder:text-base-content/30"
      />
    </label>
  );
}

function shortCommit(commit: string): string {
  return commit.slice(0, 8);
}

function ResultSummary({ result }: { result: AnalyzeFileChangesResponse }) {
  const functionCount = result.files.reduce((sum, file) => sum + file.functions.length, 0);
  return (
    <div className="space-y-1 border-b border-base-300 bg-base-100 px-4 py-2 font-mono text-[11px] text-base-content/70">
      <p>
        {result.files.length} changed {result.files.length === 1 ? "file" : "files"},{" "}
        {functionCount} changed {functionCount === 1 ? "function" : "functions"} ·{" "}
        {shortCommit(result.base_commit)} →{" "}
        {result.head_commit ? shortCommit(result.head_commit) : "working tree"}
      </p>
      {result.skipped_files.length > 0 && (
        <details>
          <summary className="cursor-pointer">
            {result.skipped_files.length} skipped{" "}
            {result.skipped_files.length === 1 ? "file" : "files"}
          </summary>
          <ul className="mt-1 space-y-0.5 pl-3">
            {result.skipped_files.map((file) => (
              <li key={file.path}>
                {file.path} · {file.reason}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

/** Changed functions of a real repository, drawn as inputs → file → outputs. */
export default function ReviewIndex() {
  const backend = useBackend();
  const [form, setForm] = useState<ReviewForm>(loadForm);
  const [result, setResult] = useState<AnalyzeFileChangesResponse | null>(null);
  const [resultNumber, setResultNumber] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<ReviewView>("chips");
  const [outside, setOutside] = useState<OutsideMode>("dimmed");
  const latestRequest = useRef(0);

  const viewFiles = useMemo(() => (result ? toViewFiles(result.files) : []), [result]);

  function updateForm(changes: Partial<ReviewForm>) {
    setForm((current) => ({ ...current, ...changes }));
  }

  async function analyze(event: FormEvent) {
    event.preventDefault();
    const request = ++latestRequest.current;
    saveForm(form);
    setLoading(true);
    setError(null);
    try {
      const response = await backend.analyzeFileChanges({
        repo_path: form.repoPath.trim(),
        base_ref: form.baseRef.trim() || null,
        head_ref: form.headRef.trim() || null,
      });
      if (request !== latestRequest.current) return;
      setResult(response);
      setResultNumber(request);
    } catch (e) {
      if (request !== latestRequest.current) return;
      setResult(null);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (request === latestRequest.current) setLoading(false);
    }
  }

  return (
    <div>
      <form
        onSubmit={analyze}
        className="flex flex-wrap items-end gap-3 border-b border-base-300 bg-base-100 px-4 py-3"
      >
        <TextField
          label="Repository path"
          value={form.repoPath}
          placeholder="/path/to/elixir/project"
          className="min-w-72 flex-1"
          onChange={(repoPath) => updateForm({ repoPath })}
        />
        <TextField
          label="Base"
          value={form.baseRef}
          placeholder="e.g. main"
          className="w-40"
          onChange={(baseRef) => updateForm({ baseRef })}
        />
        <TextField
          label="Head"
          value={form.headRef}
          placeholder="e.g. HEAD"
          className="w-40"
          onChange={(headRef) => updateForm({ headRef })}
        />
        <button
          type="submit"
          disabled={loading || form.repoPath.trim() === ""}
          className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-medium text-white hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? "Analyzing…" : "Analyze changes"}
        </button>
        <p className="basis-full text-[11px] text-base-content/50">
          Both refs empty: uncommitted changes against HEAD. Head only: that commit against its
          parent. Base only: the working tree against the base.
        </p>
      </form>

      {error && (
        <p className="border-b border-base-300 bg-rose-500/10 px-4 py-3 font-mono text-xs text-rose-700 dark:text-rose-300">
          {error}
        </p>
      )}

      {!result && !error && !loading && (
        <p className="px-6 py-10 text-sm text-base-content/50">
          Enter the path of a git repository with Elixir code, then analyze its changes.
        </p>
      )}

      {result && (
        <>
          <ResultSummary result={result} />
          {result.files.length === 0 ? (
            <p className="px-6 py-10 text-sm text-base-content/50">
              No functions were added, changed or removed.
            </p>
          ) : (
            <>
              <Toolbar>
                <SegmentedControl
                  label="View"
                  value={view}
                  onChange={setView}
                  options={REVIEW_VIEW_OPTIONS}
                />
                <SegmentedControl
                  label="Dependencies and standard library"
                  value={outside}
                  onChange={setOutside}
                  options={OUTSIDE_MODE_OPTIONS}
                />
              </Toolbar>
              <ReviewCanvas
                files={viewFiles}
                view={view}
                outside={outside}
                resultKey={String(resultNumber)}
              />
              <Legend
                items={[
                  { label: "stale caller", swatch: <MarkerIcon marker="stale" /> },
                  {
                    label: "also changed, click to jump",
                    swatch: <MarkerIcon marker="also_changed" />,
                  },
                ]}
              />
            </>
          )}
        </>
      )}
    </div>
  );
}

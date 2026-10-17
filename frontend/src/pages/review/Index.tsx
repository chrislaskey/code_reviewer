import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { useBackend } from "../../api/context";
import type {
  AnalyzeFileChangesResponse,
  ListBranchesAndRecentCommitsResponse,
} from "../../api/types";
import { type OutsideMode } from "../../explorations/data";
import { MarkerIcon } from "../../explorations/nodes";
import { SegmentedControl, Toolbar } from "../../explorations/controls";
import { Legend } from "../../explorations/scaffold";
import {
  REVIEW_VIEW_OPTIONS,
  ReviewCanvas,
  type ReviewView,
} from "../../review/ReviewCanvas";
import { CompareFormView } from "../../review/CompareFormView";
import {
  buildRequest,
  loadForm,
  saveForm,
  withValidChoices,
  type CompareForm,
} from "../../review/compare_form";
import { toViewFiles } from "../../review/view_model";

const OUTSIDE_OPTIONS: { value: OutsideMode; label: string }[] = [
  { value: "dimmed", label: "Dimmed" },
  { value: "hidden", label: "Hidden" },
];

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
  const [form, setForm] = useState<CompareForm>(loadForm);
  const [refs, setRefs] = useState<ListBranchesAndRecentCommitsResponse | null>(null);
  const [refsError, setRefsError] = useState<string | null>(null);
  const [result, setResult] = useState<AnalyzeFileChangesResponse | null>(null);
  const [resultNumber, setResultNumber] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<ReviewView>("chips");
  const [outside, setOutside] = useState<OutsideMode>("hidden");
  const latestRequest = useRef(0);

  const viewFiles = useMemo(() => (result ? toViewFiles(result.files) : []), [result]);

  // Read the branches and commits once the path stops changing.
  useEffect(() => {
    const repoPath = form.repoPath.trim();
    if (repoPath === "") {
      setRefs(null);
      setRefsError(null);
      return;
    }
    let outdated = false;
    const timer = window.setTimeout(async () => {
      try {
        const response = await backend.listBranchesAndRecentCommits({ repo_path: repoPath });
        if (outdated) return;
        setRefs(response);
        setRefsError(null);
        setForm((current) => withValidChoices(current, response));
      } catch (e) {
        if (outdated) return;
        setRefs(null);
        setRefsError(e instanceof Error ? e.message : String(e));
      }
    }, 400);
    return () => {
      outdated = true;
      window.clearTimeout(timer);
    };
  }, [backend, form.repoPath]);

  function updateForm(changes: Partial<CompareForm>) {
    setForm((current) => ({ ...current, ...changes }));
  }

  async function analyze(event: FormEvent) {
    event.preventDefault();
    const analyzeRequest = buildRequest(form);
    if (analyzeRequest === null) return;
    const request = ++latestRequest.current;
    saveForm(form);
    setLoading(true);
    setError(null);
    try {
      const response = await backend.analyzeFileChanges(analyzeRequest);
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
      <CompareFormView
        form={form}
        refs={refs}
        refsError={refsError}
        loading={loading}
        onChange={updateForm}
        onSubmit={analyze}
      />

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
                  options={OUTSIDE_OPTIONS}
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

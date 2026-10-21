import { useEffect, useState } from "react";
import type { BackendAdapter } from "../api/adapter";
import type { ListBranchesAndRecentCommitsResponse } from "../api/types";
import { loadForm, withValidChoices, type CompareForm } from "./compare_form";

/**
 * The compare form with the branches and commits of the typed repository
 * path, read once the path stops changing.
 */
export function useCompareForm(backend: BackendAdapter) {
  const [form, setForm] = useState<CompareForm>(loadForm);
  const [refs, setRefs] = useState<ListBranchesAndRecentCommitsResponse | null>(null);
  const [refsError, setRefsError] = useState<string | null>(null);

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

  return { form, refs, refsError, updateForm };
}

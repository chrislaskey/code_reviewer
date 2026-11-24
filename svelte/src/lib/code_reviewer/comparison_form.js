// What to compare: the repository and the two sides, as the new review and
// new trace pages take them, how the form becomes the core crate's
// `AnalyzeChangesRequest` fields (repo_path, base_ref, head_ref,
// compare_from_merge_base) and back. See lib/routes.js for how a request
// travels in a URL.

/** @typedef {"working_tree" | "commit" | "branch"} CompareMode */

export const COMPARE_MODE_OPTIONS = [
  { value: "working_tree", label: "Uncommitted changes" },
  { value: "commit", label: "One commit" },
  { value: "branch", label: "Branch vs branch" },
];

export const EMPTY_COMPARISON_FORM = {
  repoPath: "",
  mode: "working_tree",
  commitSha: "",
  headBranch: "",
  baseBranch: "",
};

/** The form as it was last submitted under `storageKey`, so a page opens on the same comparison. */
export function loadForm(storageKey, emptyForm) {
  try {
    const saved = window.localStorage.getItem(storageKey);
    return saved ? { ...emptyForm, ...JSON.parse(saved) } : { ...emptyForm };
  } catch {
    return { ...emptyForm };
  }
}

export function saveForm(storageKey, form) {
  try {
    window.localStorage.setItem(storageKey, JSON.stringify(form));
  } catch {
    // Remembering the form is a convenience only.
  }
}

/** Keeps choices that still exist in the repository, and picks sensible ones for the rest. */
export function withValidChoices(form, refs) {
  const commitSha = refs.recent_commits.some((commit) => commit.sha === form.commitSha)
    ? form.commitSha
    : (refs.recent_commits[0]?.sha ?? "");
  const headBranch = refs.branches.includes(form.headBranch)
    ? form.headBranch
    : (refs.current_branch ?? refs.branches[0] ?? "");
  const baseBranch = refs.branches.includes(form.baseBranch)
    ? form.baseBranch
    : (refs.default_branch ?? refs.branches.find((branch) => branch !== headBranch) ?? "");
  return { ...form, commitSha, headBranch, baseBranch };
}

/** The comparison fields of a request, or null while the form is missing something. */
export function buildComparison(form) {
  const repo_path = form.repoPath.trim();
  if (repo_path === "") return null;
  switch (form.mode) {
    case "working_tree":
      return { repo_path };
    case "commit":
      return form.commitSha ? { repo_path, head_ref: form.commitSha } : null;
    case "branch":
      return form.headBranch && form.baseBranch && form.headBranch !== form.baseBranch
        ? { repo_path, base_ref: form.baseBranch, head_ref: form.headBranch, compare_from_merge_base: true }
        : null;
    default:
      return null;
  }
}

/** The form fields that build a request's comparison, to edit one that arrived as a link. */
export function comparisonFormFromRequest(request) {
  const mode = request.base_ref && request.head_ref ? "branch" : request.head_ref ? "commit" : "working_tree";
  return {
    repoPath: request.repo_path,
    mode,
    commitSha: mode === "commit" ? request.head_ref : "",
    headBranch: mode === "branch" ? request.head_ref : "",
    baseBranch: mode === "branch" ? request.base_ref : "",
  };
}

/** A sentence saying what the form will compare. */
export function describeComparison(form) {
  switch (form.mode) {
    case "working_tree":
      return "Everything changed but not yet committed, compared with the last commit.";
    case "commit":
      return "What the chosen commit changed, compared with the commit before it.";
    case "branch":
      if (form.headBranch && form.headBranch === form.baseBranch) {
        return "Pick two different branches.";
      }
      return `What ${form.headBranch || "the branch"} changed since it branched off ${form.baseBranch || "the other branch"}, like a pull request.`;
    default:
      return "";
  }
}

/** What a request compares, for the summary line. */
export function describeRequest(request) {
  if (request.base_ref && request.head_ref) return `${request.head_ref} since it branched off ${request.base_ref}`;
  if (request.head_ref) return `commit ${request.head_ref.slice(0, 8)}`;
  return "uncommitted changes";
}

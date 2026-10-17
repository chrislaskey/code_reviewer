import type {
  AnalyzeFileChangesRequest,
  ListBranchesAndRecentCommitsResponse,
} from "../api/types";

/** What to review: the form on the review page, as the user fills it in. */

export type CompareMode = "working_tree" | "commit" | "branch";

export const COMPARE_MODE_OPTIONS: { value: CompareMode; label: string }[] = [
  { value: "working_tree", label: "Uncommitted changes" },
  { value: "commit", label: "One commit" },
  { value: "branch", label: "Branch vs branch" },
];

export interface CompareForm {
  repoPath: string;
  mode: CompareMode;
  commitSha: string;
  headBranch: string;
  baseBranch: string;
  /** One pattern per line. */
  includeRules: string;
  ignoreRules: string;
}

export const EMPTY_FORM: CompareForm = {
  repoPath: "",
  mode: "working_tree",
  commitSha: "",
  headBranch: "",
  baseBranch: "",
  includeRules: "",
  ignoreRules: "",
};

const STORAGE_KEY = "code_reviewer.review_form.v2";

export function loadForm(): CompareForm {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return saved ? { ...EMPTY_FORM, ...JSON.parse(saved) } : EMPTY_FORM;
  } catch {
    return EMPTY_FORM;
  }
}

export function saveForm(form: CompareForm) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(form));
  } catch {
    // Remembering the form is a convenience only.
  }
}

/** Keep choices that still exist in the repository, and pick sensible ones for the rest. */
export function withValidChoices(
  form: CompareForm,
  refs: ListBranchesAndRecentCommitsResponse,
): CompareForm {
  const commitSha = refs.recent_commits.some((commit) => commit.sha === form.commitSha)
    ? form.commitSha
    : (refs.recent_commits[0]?.sha ?? "");
  const headBranch = refs.branches.includes(form.headBranch)
    ? form.headBranch
    : (refs.current_branch ?? refs.branches[0] ?? "");
  const baseBranch = refs.branches.includes(form.baseBranch)
    ? form.baseBranch
    : (refs.default_branch ??
      refs.branches.find((branch) => branch !== headBranch) ??
      "");
  return { ...form, commitSha, headBranch, baseBranch };
}

function splitLines(text: string): string[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
}

/** Null while the form is missing something. */
export function buildRequest(form: CompareForm): AnalyzeFileChangesRequest | null {
  const repo_path = form.repoPath.trim();
  if (repo_path === "") return null;
  const module_rules = {
    include: splitLines(form.includeRules),
    ignore: splitLines(form.ignoreRules),
  };

  switch (form.mode) {
    case "working_tree":
      return { repo_path, module_rules };
    case "commit":
      return form.commitSha ? { repo_path, head_ref: form.commitSha, module_rules } : null;
    case "branch":
      return form.headBranch && form.baseBranch && form.headBranch !== form.baseBranch
        ? {
            repo_path,
            base_ref: form.baseBranch,
            head_ref: form.headBranch,
            compare_from_merge_base: true,
            module_rules,
          }
        : null;
  }
}

/** A sentence saying what the form will review. */
export function describeComparison(form: CompareForm): string {
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
  }
}

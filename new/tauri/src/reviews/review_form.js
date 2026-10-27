// What to review: the form on the new review page, as the user fills it in,
// and how it becomes the request the show page runs.

/** @typedef {"working_tree" | "commit" | "branch"} CompareMode */

export const COMPARE_MODE_OPTIONS = [
  { value: "working_tree", label: "Uncommitted changes" },
  { value: "commit", label: "One commit" },
  { value: "branch", label: "Branch vs branch" },
];

export const EMPTY_FORM = {
  repoPath: "",
  mode: "working_tree",
  commitSha: "",
  headBranch: "",
  baseBranch: "",
  /** One pattern per line. */
  includeRules: "",
  ignoreRules: "",
  /** A group config file to use instead of `.code_reviewer.toml` in the repo. */
  groupConfigPath: "",
  /** One `<group>=<pattern>` per line. */
  groupIncludes: "",
  groupExcludes: "",
};

const STORAGE_KEY = "code_reviewer.review_form.v2";

/** The form as it was last submitted, so the page opens on the same review. */
export function loadForm() {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return saved ? { ...EMPTY_FORM, ...JSON.parse(saved) } : { ...EMPTY_FORM };
  } catch {
    return { ...EMPTY_FORM };
  }
}

export function saveForm(form) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(form));
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

function splitLines(text) {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
}

/** The `analyze_file_changes` request, or null while the form is missing something. */
export function buildRequest(form) {
  const repo_path = form.repoPath.trim();
  if (repo_path === "") return null;
  const module_rules = {
    include: splitLines(form.includeRules),
    ignore: splitLines(form.ignoreRules),
  };
  const group_flags = {
    group_config_path: form.groupConfigPath.trim() || null,
    include: splitLines(form.groupIncludes),
    exclude: splitLines(form.groupExcludes),
  };

  switch (form.mode) {
    case "working_tree":
      return { repo_path, module_rules, group_flags };
    case "commit":
      return form.commitSha ? { repo_path, head_ref: form.commitSha, module_rules, group_flags } : null;
    case "branch":
      return form.headBranch && form.baseBranch && form.headBranch !== form.baseBranch
        ? {
            repo_path,
            base_ref: form.baseBranch,
            head_ref: form.headBranch,
            compare_from_merge_base: true,
            module_rules,
            group_flags,
          }
        : null;
    default:
      return null;
  }
}

/** A sentence saying what the form will review. */
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

/* -- The show page's URL -------------------------------------------------------- */

const REQUEST_PARAM = "request";

/** The show page for a request. The request travels as JSON in the query. */
export function showPageUrl(request) {
  const query = new URLSearchParams({ [REQUEST_PARAM]: JSON.stringify(request) });
  return `/reviews/show.html?${query}`;
}

/** The request in the show page's URL, or null when there is none. */
export function requestFromLocation(location) {
  const raw = new URLSearchParams(location.search).get(REQUEST_PARAM);
  if (!raw) return null;
  try {
    const request = JSON.parse(raw);
    return typeof request?.repo_path === "string" ? request : null;
  } catch {
    return null;
  }
}

/** What a request compares, for the summary line. */
export function describeRequest(request) {
  if (request.base_ref && request.head_ref) return `${request.head_ref} since it branched off ${request.base_ref}`;
  if (request.head_ref) return `commit ${request.head_ref.slice(0, 8)}`;
  return "uncommitted changes";
}

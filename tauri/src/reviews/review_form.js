// What to review: the form on the new review page, as the user fills it in,
// how it becomes the request the show page runs (the core crate's
// `analyze_file_changes` request), and back. The comparison fields are the
// shared ones in lib/comparison_form.js; this adds the module rules and
// the groups.

import {
  EMPTY_COMPARISON_FORM,
  buildComparison,
  comparisonFormFromRequest,
  loadForm as loadComparisonForm,
  saveForm as saveComparisonForm,
} from "../lib/comparison_form.js";

export const EMPTY_FORM = {
  ...EMPTY_COMPARISON_FORM,
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
  return loadComparisonForm(STORAGE_KEY, EMPTY_FORM);
}

export function saveForm(form) {
  saveComparisonForm(STORAGE_KEY, form);
}

function splitLines(text) {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
}

/** The `analyze_file_changes` request, or null while the form is missing something. */
export function buildRequest(form) {
  const comparison = buildComparison(form);
  if (comparison === null) return null;
  return {
    ...comparison,
    module_rules: {
      include: splitLines(form.includeRules),
      ignore: splitLines(form.ignoreRules),
    },
    group_flags: {
      group_config_path: form.groupConfigPath.trim() || null,
      include: splitLines(form.groupIncludes),
      exclude: splitLines(form.groupExcludes),
    },
  };
}

/** The form that builds a request, to edit a review that arrived as a link. */
export function formFromRequest(request) {
  return {
    ...EMPTY_FORM,
    ...comparisonFormFromRequest(request),
    includeRules: (request.module_rules?.include ?? []).join("\n"),
    ignoreRules: (request.module_rules?.ignore ?? []).join("\n"),
    groupConfigPath: request.group_flags?.group_config_path ?? "",
    groupIncludes: (request.group_flags?.include ?? []).join("\n"),
    groupExcludes: (request.group_flags?.exclude ?? []).join("\n"),
  };
}

// The message a page shows for what a call threw.

/**
 * The message to show for what a call threw: an Error's message, so the
 * web and the app read the same.
 * @param {unknown} problem
 */
export function errorMessageOf(problem) {
  return problem instanceof Error ? problem.message : String(problem);
}
